use std::{cmp::max, collections::HashMap, ops::RangeInclusive, path::Path, sync::Arc};

use alloy_primitives::Address;
use brontes_pricing::{Protocol, SubGraphEdge};
use brontes_types::{
    constants::{USDC_ADDRESS, USDT_ADDRESS, WETH_ADDRESS},
    db::{
        address_metadata::AddressMetadata,
        address_to_protocol_info::ProtocolInfo,
        builder::BuilderInfo,
        cex::{CexPriceMap, CexQuote},
        dex::{make_filter_key_range, make_key, DexPrices, DexQuoteWithIndex, DexQuotes},
        initialized_state::{CEX_FLAG, DEX_PRICE_FLAG, META_FLAG, SKIP_FLAG, TRACE_FLAG},
        metadata::{BlockMetadata, BlockMetadataInner, Metadata},
        mev_block::MevBlockWithClassified,
        pool_creation_block::PoolsToAddresses,
        searcher::SearcherInfo,
        token_info::{TokenInfo, TokenInfoWithAddress},
        traces::TxTracesInner,
        traits::{LibmdbxReader, LibmdbxWriter},
    },
    mev::{Bundle, MevBlock},
    pair::Pair,
    structured_trace::TxTrace,
    traits::TracingProvider,
};
use eyre::eyre;
use itertools::Itertools;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use reth_db::DatabaseError;
use reth_interfaces::db::LogLevel;
use tracing::info;

use crate::{
    clickhouse::Clickhouse,
    libmdbx::{
        tables::{BlockInfo, CexPrice, DexPrice, MevBlocks, Tables, *},
        types::LibmdbxData,
        CompressedTable, Libmdbx, LibmdbxInitializer,
    },
    AddressToProtocolInfo, PoolCreationBlocks, SubGraphs, TokenDecimals, TxTraces,
};

pub struct LibmdbxReadWriter(pub Libmdbx);

impl LibmdbxReadWriter {
    pub fn init_db<P: AsRef<Path>>(path: P, log_level: Option<LogLevel>) -> eyre::Result<Self> {
        Ok(Self(Libmdbx::init_db(path, log_level)?))
    }

    /// initializes all the tables with data via the CLI
    pub async fn initialize_tables<T: TracingProvider>(
        &'static self,
        clickhouse: &'static Clickhouse,
        tracer: Arc<T>,
        tables: &[Tables],
        clear_tables: bool,
        block_range: Option<(u64, u64)>, // inclusive of start only
    ) -> eyre::Result<()> {
        let initializer = LibmdbxInitializer::new(self, clickhouse, tracer);
        initializer
            .initialize(tables, clear_tables, block_range)
            .await?;

        Ok(())
    }

    /// full range tables are tables such as token info or pool creation blocks.
    /// given that these are full range. they will be inited for such. thus,
    /// if we have any entries in the db, we will have the entrie range.
    pub fn init_full_range_tables(&self) -> bool {
        [Tables::PoolCreationBlocks, Tables::AddressToProtocolInfo, Tables::TokenDecimals]
            .into_par_iter()
            .map(|table| match table {
                Tables::AddressToProtocolInfo => self
                    .has_entry::<AddressToProtocolInfo>()
                    .unwrap_or_default(),
                Tables::TokenDecimals => self.has_entry::<TokenDecimals>().unwrap_or_default(),
                Tables::PoolCreationBlocks => {
                    self.has_entry::<PoolCreationBlocks>().unwrap_or_default()
                }
                _ => true,
            })
            .collect::<Vec<_>>()
            .iter()
            .any(|t| !*t)
    }

    fn has_entry<TB>(&self) -> eyre::Result<bool>
    where
        TB: CompressedTable,
        TB::Value: From<TB::DecompressedValue> + Into<TB::DecompressedValue>,
    {
        let tx = self.0.ro_tx()?;
        let mut cur = tx.new_cursor::<TB>()?;

        Ok(cur.next()?.is_some())
    }

    pub fn state_to_initialize(
        &self,
        start_block: u64,
        end_block: u64,
        needs_dex_price: bool,
    ) -> eyre::Result<Vec<RangeInclusive<u64>>> {
        let tx = self.0.ro_tx()?;
        let mut cur = tx.new_cursor::<InitializedState>()?;

        let mut peek_cur = cur.walk_range(start_block..=end_block)?.peekable();
        if peek_cur.peek().is_none() {
            if needs_dex_price {
                return Err(eyre::eyre!(
                    "Block is missing dex pricing, please run with flag `--run-dex-pricing`"
                ))
            }

            tracing::info!("entire range missing");

            return Ok(vec![start_block..=end_block])
        }

        let mut result = Vec::new();
        let mut block_tracking = start_block;

        for entry in peek_cur {
            if let Ok(has_info) = entry {
                let block = has_info.0;
                let state = has_info.1;
                // if we are missing the block, we add it to the range
                if block != block_tracking {
                    tracing::info!(block, block_tracking, "block != tracking");
                    result.push(block_tracking..=block);
                    block_tracking = block + 1;

                    if needs_dex_price {
                        return Err(eyre::eyre!(
                            "Block is missing dex pricing, please run with flag \
                             `--run-dex-pricing`"
                        ))
                    }

                    continue
                }

                block_tracking += 1;
                if needs_dex_price && !state.has_dex_price() && !state.should_ignore() {
                    tracing::error!("block is missing dex pricing");
                    return Err(eyre::eyre!(
                        "Block is missing dex pricing, please run with flag `--run-dex-pricing`"
                    ))
                }

                if !state.is_init() {
                    tracing::info!(?state, "state isn't init");
                    result.push(block..=block);
                }
            } else {
                // should never happen unless a courput db
                panic!("database is corrupted");
            }
        }

        if block_tracking - 1 != end_block {
            if needs_dex_price {
                tracing::error!("block is missing dex pricing");
                return Err(eyre::eyre!(
                    "Block is missing dex pricing, please run with flag `--run-dex-pricing`"
                ))
            }

            result.push(block_tracking - 1..=end_block);
        }

        Ok(result)
    }
}

impl LibmdbxReader for LibmdbxReadWriter {
    fn get_dex_quotes(&self, block: u64) -> eyre::Result<DexQuotes> {
        self.fetch_dex_quotes(block)
    }

    fn load_trace(&self, block_num: u64) -> eyre::Result<Vec<TxTrace>> {
        let tx = self.0.ro_tx()?;
        tx.get::<TxTraces>(block_num)?
            .ok_or_else(|| eyre::eyre!("missing trace for block: {}", block_num))
            .map(|i| {
                i.traces
                    .ok_or_else(|| eyre::eyre!("missing trace for block: {}", block_num))
            })?
    }

    fn get_protocol_details(&self, address: Address) -> eyre::Result<ProtocolInfo> {
        let tx = self.0.ro_tx()?;
        tx.get::<AddressToProtocolInfo>(address)?
            .ok_or_else(|| eyre::eyre!("entry for key {:?} in AddressToProtocolInfo", address))
    }

    fn get_metadata_no_dex_price(&self, block_num: u64) -> eyre::Result<Metadata> {
        let block_meta = self.fetch_block_metadata(block_num)?;
        self.init_state_updating(block_num, META_FLAG)?;
        let cex_quotes = self.fetch_cex_quotes(block_num)?;
        self.init_state_updating(block_num, CEX_FLAG)?;
        let eth_prices = determine_eth_prices(&cex_quotes);

        Ok(BlockMetadata::new(
            block_num,
            block_meta.block_hash,
            block_meta.block_timestamp,
            block_meta.relay_timestamp,
            block_meta.p2p_timestamp,
            block_meta.proposer_fee_recipient,
            block_meta.proposer_mev_reward,
            max(eth_prices.price.0, eth_prices.price.1),
            block_meta.private_flow.into_iter().collect(),
        )
        .into_metadata(cex_quotes, None))
    }

    fn get_metadata(&self, block_num: u64) -> eyre::Result<Metadata> {
        let block_meta = self.fetch_block_metadata(block_num)?;
        self.init_state_updating(block_num, META_FLAG)?;
        let cex_quotes = self.fetch_cex_quotes(block_num)?;
        self.init_state_updating(block_num, CEX_FLAG)?;
        let eth_prices = determine_eth_prices(&cex_quotes);
        let dex_quotes = self.fetch_dex_quotes(block_num)?;

        Ok({
            BlockMetadata::new(
                block_num,
                block_meta.block_hash,
                block_meta.block_timestamp,
                block_meta.relay_timestamp,
                block_meta.p2p_timestamp,
                block_meta.proposer_fee_recipient,
                block_meta.proposer_mev_reward,
                max(eth_prices.price.0, eth_prices.price.1),
                block_meta.private_flow.into_iter().collect(),
            )
            .into_metadata(cex_quotes, Some(dex_quotes))
        })
    }

    fn try_fetch_token_info(&self, address: Address) -> eyre::Result<TokenInfoWithAddress> {
        let tx = self.0.ro_tx()?;
        tx.get::<TokenDecimals>(address)?
            .map(|inner| TokenInfoWithAddress { inner, address })
            .ok_or_else(|| eyre::eyre!("entry for key {:?} in TokenDecimals", address))
    }

    fn try_fetch_searcher_info(&self, searcher_eoa: Address) -> eyre::Result<SearcherInfo> {
        let tx = self.0.ro_tx()?;
        tx.get::<Searcher>(searcher_eoa)?
            .ok_or_else(|| eyre::eyre!("entry for key {:?} in SearcherInfo", searcher_eoa))
    }

    fn protocols_created_before(
        &self,
        block_num: u64,
    ) -> eyre::Result<HashMap<(Address, Protocol), Pair>> {
        let tx = self.0.ro_tx()?;

        let mut cursor = tx.cursor_read::<PoolCreationBlocks>()?;
        let mut map = HashMap::default();

        for result in cursor.walk_range(0..=block_num)? {
            let res = result?.1;
            for addr in res.0.into_iter() {
                let Some(protocol_info) = tx.get::<AddressToProtocolInfo>(addr)? else {
                    continue;
                };

                map.insert(
                    (addr, protocol_info.protocol),
                    Pair(protocol_info.token0, protocol_info.token1),
                );
            }
        }

        info!(target:"brontes-libmdbx", "loaded {} pairs before block: {}", map.len(), block_num);

        Ok(map)
    }

    fn protocols_created_range(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> eyre::Result<HashMap<u64, Vec<(Address, Protocol, Pair)>>> {
        let tx = self.0.ro_tx()?;

        let mut cursor = tx.cursor_read::<PoolCreationBlocks>()?;
        let mut map = HashMap::default();

        for result in cursor.walk_range(start_block..end_block)? {
            let result = result?;
            let (block, res) = (result.0, result.1);
            for addr in res.0.into_iter() {
                let Some(protocol_info) = tx.get::<AddressToProtocolInfo>(addr)? else {
                    continue;
                };
                map.entry(block).or_insert(vec![]).push((
                    addr,
                    protocol_info.protocol,
                    Pair(protocol_info.token0, protocol_info.token1),
                ));
            }
        }
        info!(target:"brontes-libmdbx", "loaded {} pairs range: {}..{}", map.len(), start_block, end_block);

        Ok(map)
    }

    fn try_load_pair_before(
        &self,
        block: u64,
        pair: Pair,
    ) -> eyre::Result<(Pair, Vec<SubGraphEdge>)> {
        let tx = self.0.ro_tx()?;
        let subgraphs = tx
            .get::<SubGraphs>(pair.ordered())?
            .ok_or_else(|| eyre::eyre!("no subgraph found"))?;

        // if we have dex prices for a block then we have a subgraph for the block
        let (start_key, end_key) = make_filter_key_range(block);
        if tx
            .new_cursor::<DexPrice>()?
            .walk_range(start_key..=end_key)?
            .all(|f| f.is_err())
        {
            return Err(eyre::eyre!("subgraph not inited at this block range"))
        }

        let mut last: Option<(Pair, Vec<SubGraphEdge>)> = None;

        for (cur_block, update) in subgraphs.0 {
            if cur_block > block {
                break
            }
            last = Some((pair, update))
        }

        last.ok_or_else(|| eyre::eyre!("no pair found"))
    }

    fn try_fetch_address_metadata(&self, address: Address) -> eyre::Result<AddressMetadata> {
        self.0
            .ro_tx()?
            .get::<AddressMeta>(address)?
            .ok_or_else(|| eyre::eyre!("entry for key {:?} in address metadata", address))
    }

    fn try_fetch_builder_info(&self, builder_coinbase_addr: Address) -> eyre::Result<BuilderInfo> {
        self.0
            .ro_tx()?
            .get::<Builder>(builder_coinbase_addr)?
            .ok_or_else(|| eyre::eyre!("entry for key {:?} in builder info", builder_coinbase_addr))
    }
}

impl LibmdbxWriter for LibmdbxReadWriter {
    fn write_searcher_info(
        &self,
        searcher_eoa: Address,
        searcher_info: SearcherInfo,
    ) -> eyre::Result<()> {
        let data = SearcherData::new(searcher_eoa, searcher_info);
        self.0.write_table::<Searcher, SearcherData>(&vec![data])?;
        Ok(())
    }

    fn save_mev_blocks(
        &self,
        block_number: u64,
        block: MevBlock,
        mev: Vec<Bundle>,
    ) -> eyre::Result<()> {
        let data = MevBlocksData::new(block_number, MevBlockWithClassified { block, mev });

        self.0
            .write_table::<MevBlocks, MevBlocksData>(&vec![data])?;
        Ok(())
    }

    fn write_dex_quotes(&self, block_num: u64, quotes: Option<DexQuotes>) -> eyre::Result<()> {
        if let Some(quotes) = quotes {
            self.init_state_updating(block_num, DEX_PRICE_FLAG)?;
            let data = quotes
                .0
                .into_iter()
                .enumerate()
                .filter_map(|(idx, value)| value.map(|v| (idx, v)))
                .map(|(idx, value)| {
                    let index = DexQuoteWithIndex {
                        tx_idx: idx as u16,
                        quote:  value.into_iter().collect_vec(),
                    };

                    DexPriceData::new(make_key(block_num, idx as u16), index)
                })
                .collect::<Vec<_>>();

            self.0.update_db(|tx| {
                let mut cursor = tx.cursor_write::<DexPrice>()?;

                data.into_iter()
                    .map(|entry| {
                        let entry = entry.into_key_val();
                        cursor.upsert(entry.key, entry.value)?;
                        Ok(())
                    })
                    .collect::<Result<Vec<_>, DatabaseError>>()
            })??;
        }

        Ok(())
    }

    fn write_token_info(&self, address: Address, decimals: u8, symbol: String) -> eyre::Result<()> {
        Ok(self
            .0
            .write_table::<TokenDecimals, TokenDecimalsData>(&vec![TokenDecimalsData::new(
                address,
                TokenInfo::new(decimals, symbol),
            )])?)
    }

    fn save_pair_at(&self, block: u64, pair: Pair, edges: Vec<SubGraphEdge>) -> eyre::Result<()> {
        let tx = self.0.ro_tx()?;
        if let Some(mut entry) = tx.get::<SubGraphs>(pair.ordered())? {
            entry.0.insert(block, edges.into_iter().collect::<Vec<_>>());

            let data = SubGraphsData::new(pair, entry);
            self.0
                .write_table::<SubGraphs, SubGraphsData>(&vec![data])?;
        }

        Ok(())
    }

    fn insert_pool(
        &self,
        block: u64,
        address: Address,
        tokens: [Address; 2],
        classifier_name: Protocol,
    ) -> eyre::Result<()> {
        // add to default table
        self.0
            .write_table::<AddressToProtocolInfo, AddressToProtocolInfoData>(&vec![
                AddressToProtocolInfoData::new(
                    address,
                    ProtocolInfo {
                        protocol:   classifier_name,
                        init_block: block,
                        token0:     tokens[0],
                        token1:     tokens[1],
                        token2:     None,
                        token3:     None,
                        token4:     None,
                    },
                ),
            ])?;

        // add to pool creation block
        let tx = self.0.ro_tx()?;
        let mut addrs = tx
            .get::<PoolCreationBlocks>(block)?
            .map(|i| i.0)
            .unwrap_or(vec![]);

        addrs.push(address);
        self.0
            .write_table::<PoolCreationBlocks, PoolCreationBlocksData>(&vec![
                PoolCreationBlocksData::new(block, PoolsToAddresses(addrs)),
            ])?;

        Ok(())
    }

    fn save_traces(&self, block: u64, traces: Vec<TxTrace>) -> eyre::Result<()> {
        let table = TxTracesData::new(block, TxTracesInner { traces: Some(traces) });
        self.0.write_table(&vec![table])?;

        self.init_state_updating(block, TRACE_FLAG)
    }
}

impl LibmdbxReadWriter {
    fn init_state_updating(&self, block: u64, flag: u8) -> eyre::Result<()> {
        let tx = self.0.ro_tx()?;
        let mut state = tx.get::<InitializedState>(block)?.unwrap_or_default();
        state.set(flag);
        self.0
            .write_table::<InitializedState, InitializedStateData>(&vec![
                InitializedStateData::new(block, state),
            ])?;

        Ok(())
    }

    pub fn inited_range(&self, range: RangeInclusive<u64>, flag: u8) -> eyre::Result<()> {
        for block in range {
            self.init_state_updating(block, flag)?;
        }
        Ok(())
    }

    fn fetch_block_metadata(&self, block_num: u64) -> eyre::Result<BlockMetadataInner> {
        let tx = self.0.ro_tx()?;
        let res = tx
            .get::<BlockInfo>(block_num)?
            .ok_or_else(|| eyre!("Failed to fetch Metadata's block info for block {}", block_num));

        if res.is_err() {
            self.init_state_updating(block_num, SKIP_FLAG)?;
        }
        res
    }

    fn fetch_cex_quotes(&self, block_num: u64) -> eyre::Result<CexPriceMap> {
        let tx = self.0.ro_tx()?;
        let res = tx
            .get::<CexPrice>(block_num)?
            .ok_or_else(|| eyre!("Failed to fetch cexquotes's for block {}", block_num))
            .map(|e| e.0);

        if res.is_err() {
            self.init_state_updating(block_num, SKIP_FLAG)?;
        }

        Ok(CexPriceMap(res?))
    }

    pub fn fetch_dex_quotes(&self, block_num: u64) -> eyre::Result<DexQuotes> {
        let mut dex_quotes: Vec<Option<HashMap<Pair, DexPrices>>> = Vec::new();
        let (start_range, end_range) = make_filter_key_range(block_num);
        let tx = self.0.ro_tx()?;

        tx.cursor_read::<DexPrice>()?
            .walk_range(start_range..=end_range)?
            .for_each(|inner| {
                if let Ok((_, val)) = inner.map(|row| (row.0, row.1)) {
                    for _ in dex_quotes.len()..=val.tx_idx as usize {
                        dex_quotes.push(None);
                    }

                    let tx = dex_quotes.get_mut(val.tx_idx as usize).unwrap();

                    if let Some(tx) = tx.as_mut() {
                        for (pair, price) in val.quote {
                            tx.insert(pair, price);
                        }
                    } else {
                        let mut tx_pairs = HashMap::default();
                        for (pair, price) in val.quote {
                            tx_pairs.insert(pair, price);
                        }
                        *tx = Some(tx_pairs);
                    }
                }
            });

        Ok(DexQuotes(dex_quotes))
    }
}

fn determine_eth_prices(cex_quotes: &CexPriceMap) -> CexQuote {
    if let Some(eth_usdt) = cex_quotes.get_binance_quote(&Pair(WETH_ADDRESS, USDT_ADDRESS)) {
        eth_usdt
    } else {
        cex_quotes
            .get_binance_quote(&Pair(WETH_ADDRESS, USDC_ADDRESS))
            .unwrap_or_default()
    }
}
