#[cfg(feature = "local-clickhouse")]
use std::str::FromStr;
use std::{
    cmp::max,
    ops::{Bound, RangeInclusive},
    path::Path,
    sync::Arc,
};

use alloy_primitives::Address;
use brontes_pricing::{Protocol, SubGraphEdge};
#[cfg(feature = "cex-dex-markout")]
use brontes_types::db::{cex_trades::CexTradeMap, initialized_state::CEX_TRADES_FLAG};
use brontes_types::{
    constants::{ETH_ADDRESS, USDC_ADDRESS, USDT_ADDRESS, WETH_ADDRESS},
    db::{
        address_metadata::AddressMetadata,
        address_to_protocol_info::ProtocolInfo,
        builder::BuilderInfo,
        cex::{CexPriceMap, CexQuote},
        dex::{make_filter_key_range, make_key, DexPrices, DexQuoteWithIndex, DexQuotes},
        initialized_state::{
            InitializedStateMeta, CEX_QUOTES_FLAG, DEX_PRICE_FLAG, META_FLAG, SKIP_FLAG, TRACE_FLAG,
        },
        metadata::{BlockMetadata, BlockMetadataInner, Metadata},
        mev_block::MevBlockWithClassified,
        pool_creation_block::PoolsToAddresses,
        searcher::SearcherInfo,
        token_info::{TokenInfo, TokenInfoWithAddress},
        traces::TxTracesInner,
        traits::{DBWriter, LibmdbxReader},
    },
    mev::{Bundle, MevBlock},
    normalized_actions::Actions,
    pair::Pair,
    structured_trace::TxTrace,
    traits::TracingProvider,
    BlockTree, FastHashMap, SubGraphsEntry,
};
#[cfg(feature = "local-clickhouse")]
use db_interfaces::Database;
use eyre::{eyre, ErrReport};
use futures::Future;
#[cfg(feature = "local-clickhouse")]
use futures::{FutureExt, StreamExt};
use indicatif::ProgressBar;
use itertools::Itertools;
use reth_db::DatabaseError;
use reth_interfaces::db::LogLevel;
use tracing::info;

#[cfg(feature = "local-clickhouse")]
use crate::clickhouse::{
    MIN_MAX_ADDRESS_TO_PROTOCOL, MIN_MAX_POOL_CREATION_BLOCKS, MIN_MAX_TOKEN_DECIMALS,
};
#[cfg(feature = "local-clickhouse")]
use crate::libmdbx::CompressedTable;
use crate::{
    clickhouse::ClickhouseHandle,
    libmdbx::{tables::*, types::LibmdbxData, Libmdbx, LibmdbxInitializer},
};

pub trait LibmdbxInit: LibmdbxReader + DBWriter {
    /// initializes all the tables with data via the CLI
    fn initialize_tables<T: TracingProvider, CH: ClickhouseHandle>(
        &'static self,
        clickhouse: &'static CH,
        tracer: Arc<T>,
        tables: &[Tables],
        clear_tables: bool,
        block_range: Option<(u64, u64)>,
        progress_bar: Arc<Vec<(Tables, ProgressBar)>>,
    ) -> impl Future<Output = eyre::Result<()>> + Send;

    /// initializes all the tables with missing data ranges via the CLI
    fn initialize_tables_arbitrary<T: TracingProvider, CH: ClickhouseHandle>(
        &'static self,
        clickhouse: &'static CH,
        tracer: Arc<T>,
        tables: &[Tables],
        block_range: Vec<u64>,
        progress_bar: Arc<Vec<(Tables, ProgressBar)>>,
    ) -> impl Future<Output = eyre::Result<()>> + Send;

    /// checks the min and max values of the clickhouse db and sees if the full
    /// range tables have the values.
    fn init_full_range_tables<CH: ClickhouseHandle>(
        &self,
        clickhouse: &'static CH,
    ) -> impl Future<Output = bool> + Send;

    fn state_to_initialize(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> eyre::Result<Vec<(Option<Vec<Tables>>, RangeInclusive<u64>)>>;
}

pub struct LibmdbxReadWriter(pub Libmdbx);

impl LibmdbxReadWriter {
    pub fn init_db<P: AsRef<Path>>(path: P, log_level: Option<LogLevel>) -> eyre::Result<Self> {
        Ok(Self(Libmdbx::init_db(path, log_level)?))
    }

    #[cfg(feature = "local-clickhouse")]
    async fn has_clickhouse_min_max<TB, CH: ClickhouseHandle>(
        &self,
        query: &str,
        clickhouse: &'static CH,
    ) -> eyre::Result<bool>
    where
        TB: CompressedTable,
        TB::Value: From<TB::DecompressedValue> + Into<TB::DecompressedValue>,
        <TB as reth_db::table::Table>::Key: FromStr + Send + Sync,
    {
        let (min, max) = clickhouse
            .inner()
            .query_one::<(String, String), _>(query, &())
            .await?;

        let Ok(min_parsed) = min.parse::<TB::Key>() else {
            return Ok(false);
        };

        let Ok(max_parsed) = max.parse::<TB::Key>() else {
            return Ok(false);
        };

        let tx = self.0.ro_tx()?;
        let mut cur = tx.new_cursor::<TB>()?;

        let Some(has_min) = cur.first()?.map(|v| v.0 <= min_parsed) else {
            return Ok(false);
        };
        let Some(has_max) = cur.last()?.map(|v| v.0 >= max_parsed) else {
            return Ok(false);
        };

        Ok(has_min && has_max)
    }
}

impl LibmdbxInit for LibmdbxReadWriter {
    /// initializes all the tables with data via the CLI
    async fn initialize_tables<T: TracingProvider, CH: ClickhouseHandle>(
        &'static self,
        clickhouse: &'static CH,
        tracer: Arc<T>,
        tables: &[Tables],
        clear_tables: bool,
        block_range: Option<(u64, u64)>, // inclusive of start only
        progress_bar: Arc<Vec<(Tables, ProgressBar)>>,
    ) -> eyre::Result<()> {
        let initializer = LibmdbxInitializer::new(self, clickhouse, tracer);
        initializer
            .initialize(tables, clear_tables, block_range, progress_bar)
            .await?;

        Ok(())
    }

    /// initializes all the tables with missing data ranges via the CLI
    async fn initialize_tables_arbitrary<T: TracingProvider, CH: ClickhouseHandle>(
        &'static self,
        clickhouse: &'static CH,
        tracer: Arc<T>,
        tables: &[Tables],
        block_range: Vec<u64>,
        progress_bar: Arc<Vec<(Tables, ProgressBar)>>,
    ) -> eyre::Result<()> {
        let block_range = Box::leak(Box::new(block_range));

        let initializer = LibmdbxInitializer::new(self, clickhouse, tracer);
        initializer
            .initialize_arbitrary_state(tables, block_range, progress_bar)
            .await?;

        Ok(())
    }

    /// checks the min and max values of the clickhouse db and sees if the full
    /// range tables have the values.
    #[cfg(feature = "local-clickhouse")]
    async fn init_full_range_tables<CH: ClickhouseHandle>(&self, clickhouse: &'static CH) -> bool {
        futures::stream::iter([
            Tables::PoolCreationBlocks,
            Tables::AddressToProtocolInfo,
            Tables::TokenDecimals,
        ])
        .map(|table| async move {
            match table {
                Tables::AddressToProtocolInfo => self
                    .has_clickhouse_min_max::<AddressToProtocolInfo, CH>(
                        MIN_MAX_ADDRESS_TO_PROTOCOL,
                        clickhouse,
                    )
                    .await
                    .unwrap_or_default(),
                Tables::TokenDecimals => self
                    .has_clickhouse_min_max::<TokenDecimals, CH>(MIN_MAX_TOKEN_DECIMALS, clickhouse)
                    .await
                    .unwrap_or_default(),
                Tables::PoolCreationBlocks => self
                    .has_clickhouse_min_max::<PoolCreationBlocks, CH>(
                        MIN_MAX_POOL_CREATION_BLOCKS,
                        clickhouse,
                    )
                    .await
                    .unwrap_or_default(),
                _ => true,
            }
        })
        .any(|f| f.map(|f| !f))
        .await
    }

    #[cfg(not(feature = "local-clickhouse"))]
    async fn init_full_range_tables<CH: ClickhouseHandle>(&self, _clickhouse: &'static CH) -> bool {
        true
    }

    fn state_to_initialize(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> eyre::Result<Vec<(Option<Vec<Tables>>, RangeInclusive<u64>)>> {
        let mut tables_to_init = vec![Tables::BlockInfo];
        #[cfg(not(feature = "sorella-server"))]
        tables_to_init.push(Tables::TxTraces);
        #[cfg(not(feature = "cex-dex-markout"))]
        tables_to_init.push(Tables::CexPrice);
        #[cfg(feature = "cex-dex-markout")]
        tables_to_init.push(Tables::CexTrades);

        let tx = self.0.ro_tx()?;
        let mut cur = tx.new_cursor::<InitializedState>()?;

        let mut peek_cur = cur.walk_range(start_block..=end_block)?.peekable();
        if peek_cur.peek().is_none() {
            tracing::info!("entire range missing");
            return Ok(vec![(Some(tables_to_init), start_block..=end_block)])
        }

        let mut result: Vec<(Vec<Tables>, RangeInclusive<u64>)> = Vec::new();
        let mut block_tracking = start_block;
        let mut oldest_missing_data_block_continuous: Option<u64> = None;

        for entry in peek_cur {
            if let Ok(has_info) = entry {
                let block = has_info.0;
                let state = has_info.1;
                // if we are missing the block, we add it to the range
                if block != block_tracking {
                    tracing::info!(block, block_tracking, "block != tracking");

                    if oldest_missing_data_block_continuous.is_none() {
                        result.push((tables_to_init, block_tracking..=block));
                    }

                    block_tracking = block + 1;

                    continue
                }

                // We are trying to build the largest ranges, so we need to check if the state
                // is missing, if it is we will add it as the oldest missing data block &
                // continue

                // Prob make this a hashmap by table so we have the ranges on a by table basis
                // so we can easily offload to the correct init function
                if let Some(tables) = tables_to_initialize(Some(state)) {
                    if oldest_missing_data_block_continuous.is_none() {
                        oldest_missing_data_block_continuous = Some(block_tracking);
                    }
                    if block_tracking == end_block {
                        result.push((
                            tables_to_init,
                            oldest_missing_data_block_continuous.unwrap()..=block,
                        ));
                        oldest_missing_data_block_continuous = None;
                    }
                }
                block_tracking += 1;
            } else {
                // should never happen unless db is corrupted
                panic!("database is corrupted");
            }
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
        self.init_state_updating(block_num, CEX_QUOTES_FLAG)?;
        let eth_prices = determine_eth_prices(&cex_quotes);
        #[cfg(feature = "cex-dex-markout")]
        let trades = self.fetch_trades(block_num).ok();

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
        .into_metadata(
            cex_quotes,
            None,
            None,
            #[cfg(feature = "cex-dex-markout")]
            trades,
        ))
    }

    fn get_metadata(&self, block_num: u64) -> eyre::Result<Metadata> {
        let block_meta = self.fetch_block_metadata(block_num)?;
        self.init_state_updating(block_num, META_FLAG)?;
        let cex_quotes = self.fetch_cex_quotes(block_num)?;
        self.init_state_updating(block_num, CEX_QUOTES_FLAG)?;
        let eth_prices = determine_eth_prices(&cex_quotes);
        let dex_quotes = self.fetch_dex_quotes(block_num)?;

        #[cfg(feature = "cex-dex-markout")]
        let trades = self.fetch_trades(block_num).ok();

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
            .into_metadata(
                cex_quotes,
                Some(dex_quotes),
                None,
                #[cfg(feature = "cex-dex-markout")]
                trades,
            )
        })
    }

    fn try_fetch_token_info(&self, address: Address) -> eyre::Result<TokenInfoWithAddress> {
        let tx = self.0.ro_tx()?;

        let address = if address == ETH_ADDRESS { WETH_ADDRESS } else { address };

        tx.get::<TokenDecimals>(address)?
            .map(|inner| TokenInfoWithAddress { inner, address })
            .ok_or_else(|| eyre::eyre!("entry for key {:?} in TokenDecimals", address))
    }

    fn try_fetch_searcher_eoa_info(
        &self,
        searcher_eoa: Address,
    ) -> eyre::Result<Option<SearcherInfo>> {
        self.0
            .ro_tx()?
            .get::<SearcherEOAs>(searcher_eoa)
            .map_err(ErrReport::from)
    }

    fn try_fetch_searcher_contract_info(
        &self,
        searcher_contract: Address,
    ) -> eyre::Result<Option<SearcherInfo>> {
        let tx = self.0.ro_tx()?;
        tx.get::<SearcherContracts>(searcher_contract)
            .map_err(ErrReport::from)
    }

    fn fetch_all_searcher_eoa_info(&self) -> eyre::Result<Vec<(Address, SearcherInfo)>> {
        let tx = self.0.ro_tx()?;
        let mut cursor = tx.cursor_read::<SearcherEOAs>()?;

        let mut result = Vec::new();

        // Start the walk from the first key-value pair
        let walker = cursor.walk(None)?;

        // Iterate over all the key-value pairs using the walker
        for row in walker {
            let row = row?;
            let address = row.0;
            let searcher_info = row.1;
            result.push((address, searcher_info));
        }

        Ok(result)
    }

    fn fetch_all_searcher_contract_info(&self) -> eyre::Result<Vec<(Address, SearcherInfo)>> {
        let tx = self.0.ro_tx()?;
        let mut cursor = tx.cursor_read::<SearcherContracts>()?;

        let mut result = Vec::new();

        // Start the walk from the first key-value pair
        let walker = cursor.walk(None)?;

        // Iterate over all the key-value pairs using the walker
        for row in walker {
            let row = row?;
            let address = row.0;
            let searcher_info = row.1;
            result.push((address, searcher_info));
        }

        Ok(result)
    }

    fn protocols_created_before(
        &self,
        block_num: u64,
    ) -> eyre::Result<FastHashMap<(Address, Protocol), Pair>> {
        let tx = self.0.ro_tx()?;

        let mut cursor = tx.cursor_read::<PoolCreationBlocks>()?;
        let mut map = FastHashMap::default();

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
    ) -> eyre::Result<FastHashMap<u64, Vec<(Address, Protocol, Pair)>>> {
        let tx = self.0.ro_tx()?;

        let mut cursor = tx.cursor_read::<PoolCreationBlocks>()?;
        let mut map = FastHashMap::default();

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
            .get::<SubGraphs>(pair)?
            .ok_or_else(|| eyre::eyre!("no subgraph found"))?;

        // if we have dex prices for a block then we have a subgraph for the block
        let (start_key, end_key) = make_filter_key_range(block);
        if !tx
            .new_cursor::<DexPrice>()?
            .walk_range(start_key..=end_key)?
            .all(|f| f.is_ok())
        {
            tracing::debug!(
                ?pair,
                ?block,
                "no pricing for block. cannot verify most recent subgraph is valid"
            );

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

    fn try_fetch_address_metadata(
        &self,
        address: Address,
    ) -> eyre::Result<Option<AddressMetadata>> {
        self.0
            .ro_tx()?
            .get::<AddressMeta>(address)
            .map_err(ErrReport::from)
    }

    fn try_fetch_builder_info(
        &self,
        builder_coinbase_addr: Address,
    ) -> eyre::Result<Option<BuilderInfo>> {
        self.0
            .ro_tx()?
            .get::<Builder>(builder_coinbase_addr)
            .map_err(ErrReport::from)
    }

    fn fetch_all_builder_info(&self) -> eyre::Result<Vec<(Address, BuilderInfo)>> {
        let tx = self.0.ro_tx()?;
        let mut cursor = tx.cursor_read::<Builder>()?;

        let mut result = Vec::new();

        // Start the walk from the first key-value pair
        let walker = cursor.walk(None)?;

        // Iterate over all the key-value pairs using the walker
        for row in walker {
            let row = row?;
            let address = row.0;
            let searcher_info = row.1;
            result.push((address, searcher_info));
        }

        Ok(result)
    }

    fn try_fetch_mev_blocks(
        &self,
        start_block: Option<u64>,
        end_block: u64,
    ) -> eyre::Result<Vec<MevBlockWithClassified>> {
        let tx = self.0.ro_tx()?;
        let mut cursor = tx.cursor_read::<MevBlocks>()?;
        let mut res = Vec::new();

        let range = if let Some(start) = start_block {
            (Bound::Included(start), Bound::Excluded(end_block))
        } else {
            (Bound::Unbounded, Bound::Excluded(end_block))
        };

        for entry in cursor.walk_range(range)?.flatten() {
            res.push(entry.1);
        }

        Ok(res)
    }

    fn fetch_all_mev_blocks(
        &self,
        start_block: Option<u64>,
    ) -> eyre::Result<Vec<MevBlockWithClassified>> {
        let tx = self.0.ro_tx()?;
        let mut cursor = tx.cursor_read::<MevBlocks>()?;

        let mut res = Vec::new();

        // Start the walk from the first key-value pair
        let walker = cursor.walk(start_block)?;

        // Iterate over all the key-value pairs using the walker
        for row in walker {
            res.push(row?.1);
        }

        Ok(res)
    }

    fn fetch_all_address_metadata(&self) -> eyre::Result<Vec<(Address, AddressMetadata)>> {
        let tx = self.0.ro_tx()?;
        let mut cursor = tx.cursor_read::<AddressMeta>()?;

        let mut result = Vec::new();

        // Start the walk from the first key-value pair
        let walker = cursor.walk(None)?;

        // Iterate over all the key-value pairs using the walker
        for row in walker {
            let row = row?;
            let address = row.0;
            let metadata = row.1;
            result.push((address, metadata));
        }

        Ok(result)
    }
}

impl DBWriter for LibmdbxReadWriter {
    type Inner = Self;

    fn inner(&self) -> &Self::Inner {
        unreachable!()
    }

    async fn write_searcher_info(
        &self,
        eoa_address: Address,
        contract_address: Option<Address>,
        eoa_info: SearcherInfo,
        contract_info: Option<SearcherInfo>,
    ) -> eyre::Result<()> {
        self.write_searcher_eoa_info(eoa_address, eoa_info)
            .await
            .expect("libmdbx write failure");

        if let Some(contract_address) = contract_address {
            self.write_searcher_contract_info(contract_address, contract_info.unwrap_or_default())
                .await
                .expect("libmdbx write failure");
        }
        Ok(())
    }

    async fn write_searcher_eoa_info(
        &self,
        searcher_eoa: Address,
        searcher_info: SearcherInfo,
    ) -> eyre::Result<()> {
        let data = SearcherEOAsData::new(searcher_eoa, searcher_info);
        self.0
            .write_table::<SearcherEOAs, SearcherEOAsData>(&[data])
            .expect("libmdbx write failure");
        Ok(())
    }

    async fn write_searcher_contract_info(
        &self,
        searcher_contract: Address,
        searcher_info: SearcherInfo,
    ) -> eyre::Result<()> {
        let data = SearcherContractsData::new(searcher_contract, searcher_info);
        self.0
            .write_table::<SearcherContracts, SearcherContractsData>(&[data])
            .expect("libmdbx write failure");
        Ok(())
    }

    async fn write_address_meta(
        &self,
        address: Address,
        metadata: AddressMetadata,
    ) -> eyre::Result<()> {
        let data = AddressMetaData::new(address, metadata);

        self.0
            .write_table::<AddressMeta, AddressMetaData>(&[data])
            .expect("libmdx metadata write failure");

        Ok(())
    }

    async fn save_mev_blocks(
        &self,
        block_number: u64,
        block: MevBlock,
        mev: Vec<Bundle>,
    ) -> eyre::Result<()> {
        let data = MevBlocksData::new(block_number, MevBlockWithClassified { block, mev });

        self.0
            .write_table::<MevBlocks, MevBlocksData>(&[data])
            .expect("libmdbx write failure");
        Ok(())
    }

    async fn write_dex_quotes(
        &self,
        block_num: u64,
        quotes: Option<DexQuotes>,
    ) -> eyre::Result<()> {
        if let Some(quotes) = quotes {
            self.init_state_updating(block_num, DEX_PRICE_FLAG)
                .expect("libmdbx write failure");
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

            self.0
                .update_db(|tx| {
                    let mut cursor = tx
                        .cursor_write::<DexPrice>()
                        .expect("libmdbx write failure");

                    data.into_iter()
                        .map(|entry| {
                            let entry = entry.into_key_val();
                            cursor
                                .upsert(entry.key, entry.value)
                                .expect("libmdbx write failure");
                            Ok(())
                        })
                        .collect::<Result<Vec<_>, DatabaseError>>()
                })
                .expect("libmdbx write failure")
                .expect("libmdbx write failure");
        }

        Ok(())
    }

    async fn write_token_info(
        &self,
        address: Address,
        decimals: u8,
        symbol: String,
    ) -> eyre::Result<()> {
        self.0
            .write_table::<TokenDecimals, TokenDecimalsData>(&[TokenDecimalsData::new(
                address,
                TokenInfo::new(decimals, symbol),
            )])
            .expect("libmdbx write failure");
        Ok(())
    }

    fn save_pair_at(&self, block: u64, pair: Pair, edges: Vec<SubGraphEdge>) -> eyre::Result<()> {
        let tx = self.0.ro_tx()?;

        if let Some(mut entry) = tx.get::<SubGraphs>(pair).expect("libmdbx write failure") {
            entry.0.insert(block, edges.into_iter().collect::<Vec<_>>());

            let data = SubGraphsData::new(pair, entry);
            self.0
                .write_table::<SubGraphs, SubGraphsData>(&[data])
                .expect("libmdbx write failure");
        } else {
            let mut map = FastHashMap::default();
            map.insert(block, edges);
            let subgraph_entry = SubGraphsEntry(map);
            let data = SubGraphsData::new(pair, subgraph_entry);
            self.0
                .write_table::<SubGraphs, SubGraphsData>(&[data])
                .expect("libmdbx write failure");
        }

        Ok(())
    }

    async fn insert_pool(
        &self,
        block: u64,
        address: Address,
        tokens: &[Address],
        curve_lp_token: Option<Address>,
        classifier_name: Protocol,
    ) -> eyre::Result<()> {
        // add to default table
        let mut tokens = tokens.iter();
        let default = Address::ZERO;
        self.0
            .write_table::<AddressToProtocolInfo, AddressToProtocolInfoData>(&[
                AddressToProtocolInfoData::new(
                    address,
                    ProtocolInfo {
                        protocol: classifier_name,
                        init_block: block,
                        token0: *tokens.next().unwrap_or(&default),
                        token1: *tokens.next().unwrap_or(&default),
                        token2: tokens.next().cloned(),
                        token3: tokens.next().cloned(),
                        token4: tokens.next().cloned(),
                        curve_lp_token,
                    },
                ),
            ])
            .expect("libmdbx write failure");

        // add to pool creation block
        let tx = self.0.ro_tx().expect("libmdbx write failure");
        let mut addrs = tx
            .get::<PoolCreationBlocks>(block)
            .expect("libmdbx write failure")
            .map(|i| i.0)
            .unwrap_or_default();

        addrs.push(address);
        self.0
            .write_table::<PoolCreationBlocks, PoolCreationBlocksData>(&[
                PoolCreationBlocksData::new(block, PoolsToAddresses(addrs)),
            ])
            .expect("libmdbx write failure");

        Ok(())
    }

    async fn save_traces(&self, block: u64, traces: Vec<TxTrace>) -> eyre::Result<()> {
        let table = TxTracesData::new(block, TxTracesInner { traces: Some(traces) });
        self.0.write_table(&[table]).expect("libmdbx write failure");

        self.init_state_updating(block, TRACE_FLAG)
    }

    async fn write_builder_info(
        &self,
        builder_address: Address,
        builder_info: BuilderInfo,
    ) -> eyre::Result<()> {
        let data = BuilderData::new(builder_address, builder_info);
        self.0
            .write_table::<Builder, BuilderData>(&[data])
            .expect("libmdbx write failure");
        Ok(())
    }

    /// only for internal functionality (i.e. clickhouse)
    async fn insert_tree(&self, _tree: Arc<BlockTree<Actions>>) -> eyre::Result<()> {
        Ok(())
    }
}

impl LibmdbxReadWriter {
    fn init_state_updating(&self, block: u64, flag: u8) -> eyre::Result<()> {
        let tx = self.0.ro_tx()?;
        let mut state = tx.get::<InitializedState>(block)?.unwrap_or_default();
        state.set(flag);
        self.0
            .write_table::<InitializedState, InitializedStateData>(&[InitializedStateData::new(
                block, state,
            )])?;

        Ok(())
    }

    pub fn inited_range(&self, range: RangeInclusive<u64>, flag: u8) -> eyre::Result<()> {
        let mut range_cursor = self.0.rw_tx()?.cursor_write::<InitializedState>()?;

        let mut block = *range.start();

        while let Some(mut state) = range_cursor.next()? {
            if state.0 != block {
                let mut init_state = InitializedStateMeta::default();
                init_state.set(flag);
                let value = InitializedStateData::from((block, init_state));
                range_cursor.upsert(value.key, value.value)?;
                block += 1;
            } else {
                state.1.set(flag);
                range_cursor.upsert(state.0, state.1)?;
                block += 1;
            }
        }

        Ok(())
    }

    pub fn inited_range_arbitrary(
        &self,
        range: impl Iterator<Item = u64>,
        flag: u8,
    ) -> eyre::Result<()> {
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

    #[cfg(feature = "cex-dex-markout")]
    fn fetch_trades(&self, block_num: u64) -> eyre::Result<CexTradeMap> {
        let tx = self.0.ro_tx()?;
        tx.get::<CexTrades>(block_num)?
            .ok_or_else(|| eyre!("Failed to fetch cex trades's for block {}", block_num))
    }

    fn fetch_cex_quotes(&self, block_num: u64) -> eyre::Result<CexPriceMap> {
        let tx = self.0.ro_tx()?;
        let res = tx
            .get::<CexPrice>(block_num)?
            .ok_or_else(|| eyre!("Failed to fetch cex quotes's for block {}", block_num))
            .map(|e| e.0);

        if res.is_err() {
            self.init_state_updating(block_num, SKIP_FLAG)?;
        }

        Ok(CexPriceMap(res?))
    }

    pub fn fetch_dex_quotes(&self, block_num: u64) -> eyre::Result<DexQuotes> {
        let mut dex_quotes: Vec<Option<FastHashMap<Pair, DexPrices>>> = Vec::new();
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
                        let mut tx_pairs = FastHashMap::default();
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

pub fn determine_eth_prices(cex_quotes: &CexPriceMap) -> CexQuote {
    if let Some(eth_usdt) = cex_quotes.get_binance_quote(&Pair(WETH_ADDRESS, USDT_ADDRESS)) {
        eth_usdt
    } else {
        cex_quotes
            .get_binance_quote(&Pair(WETH_ADDRESS, USDC_ADDRESS))
            .unwrap_or_default()
    }
}

pub fn tables_to_initialize(data: Option<InitializedStateMeta>) -> Option<Vec<Tables>> {
    let mut tables = Vec::new();

    if let Some(data) = data {
        if data.should_ignore() {
            return None
        }

        #[cfg(all(feature = "local-reth", not(feature = "cex-dex-markout")))]
        {
            if (data.0 & DEX_PRICE_FLAG) == 0 {
                tables.push(Tables::DexPrice);
            }
            if (self.0 & CEX_QUOTES_FLAG) == 0 {
                tables.push(Tables::CexPrice);
            }
            if (self.0 & META_FLAG) == 0 {
                tables.push(Tables::BlockInfo);
            }
        }

        #[cfg(all(not(feature = "local-reth"), feature = "cex-dex-markout"))]
        {
            if (data.0 & DEX_PRICE_FLAG) == 0 {
                tables.push(Tables::DexPrice);
            }
            if (data.0 & CEX_TRADES_FLAG) == 0 {
                tables.push(Tables::CexTrades);
            }
            if (data.0 & META_FLAG) == 0 {
                tables.push(Tables::BlockInfo);
            }
            if (data.0 & TRACE_FLAG) == 0 {
                tables.push(Tables::TxTraces);
            }
        }

        #[cfg(all(feature = "local-reth", feature = "cex-dex-markout"))]
        {
            if (data.0 & DEX_PRICE_FLAG) == 0 {
                tables.push(Tables::DexPrice);
            }
            if (data.0 & CEX_TRADES_FLAG) == 0 {
                tables.push(Tables::CexTrades);
            }
            if (data.0 & META_FLAG) == 0 {
                tables.push(Tables::BlockInfo);
            }
        }
    } else {
        #[cfg(feature = "local-reth")]
        {
            tables.push(Tables::DexPrice);
            tables.push(Tables::BlockInfo);
            #[cfg(not(feature = "cex-dex-markout"))]
            tables.push(Tables::CexPrice);

            #[cfg(feature = "cex-dex-markout")]
            tables.push(Tables::CexTrades);
        }

        #[cfg(not(feature = "local-reth"))]
        {
            #[cfg(feature = "cex-dex-markout")]
            tables.push(Tables::CexTrades);
            #[cfg(not(feature = "cex-dex-markout"))]
            tables.push(Tables::CexPrice);

            tables.push(Tables::DexPrice);
            tables.push(Tables::TxTraces);
        }
    }

    if tables.is_empty() {
        None
    } else {
        Some(tables)
    }
}
