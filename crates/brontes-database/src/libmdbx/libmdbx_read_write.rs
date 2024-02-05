use std::{cmp::max, collections::HashMap, path::Path};

use alloy_primitives::Address;
use brontes_pricing::{Protocol, SubGraphEdge};
use brontes_types::{
    constants::{USDC_ADDRESS, USDT_ADDRESS, WETH_ADDRESS},
    db::{
        address_to_tokens::PoolTokens,
        cex::{CexPriceMap, CexQuote},
        dex::{
            decompose_key, make_filter_key_range, make_key, DexPrices, DexQuoteWithIndex, DexQuotes,
        },
        metadata::{BlockMetadata, BlockMetadataInner, Metadata},
        mev_block::MevBlockWithClassified,
        pool_creation_block::PoolsToAddresses,
        token_info::{TokenInfo, TokenInfoWithAddress},
        traces::TxTracesInner,
        traits::{LibmdbxReader, LibmdbxWriter},
    },
    mev::{Bundle, MevBlock},
    pair::Pair,
    structured_trace::TxTrace,
};
use itertools::Itertools;
use reth_db::DatabaseError;
use reth_interfaces::db::LogLevel;
use reth_libmdbx::RO;
use tracing::info;

use super::cursor::CompressedCursor;
use crate::{
    libmdbx::{
        tables::{BlockInfo, CexPrice, DexPrice, MevBlocks, *},
        types::LibmdbxData,
        Libmdbx,
    },
    AddressToProtocol, AddressToTokens, CompressedTable, PoolCreationBlocks, SubGraphs,
    TokenDecimals, TxTraces,
};

pub struct LibmdbxReadWriter(pub Libmdbx);

impl LibmdbxReadWriter {
    pub fn init_db<P: AsRef<Path>>(path: P, log_level: Option<LogLevel>) -> eyre::Result<Self> {
        Ok(Self(Libmdbx::init_db(path, log_level)?))
    }

    #[cfg(not(feature = "local"))]
    pub fn valid_range_state(&self, start_block: u64, end_block: u64) -> eyre::Result<bool> {
        self.validate_metadata_and_cex(start_block, end_block)
    }

    // local also needs to have tx traces
    #[cfg(feature = "local")]
    pub fn valid_range_state(&self, start_block: u64, end_block: u64) -> eyre::Result<bool> {
        let meta_and_cex_pass = self.validate_metadata_and_cex(start_block, end_block)?;

        // local part
        let tx = self.0.ro_tx()?;
        let mut trace_cur = tx.new_cursor::<TxTraces>()?;
        let mut res =
            self.validate_range("tx traces", trace_cur, start_block, end_block, |b| *b)?;

        return Ok(meta_and_cex_pass && res)
    }

    pub fn has_dex_pricing_for_range(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> eyre::Result<bool> {
        let start_key = make_key(start_block, 0);
        let end_key = make_key(end_block, u16::MAX);
        let tx = self.0.ro_tx()?;
        let cursor = tx.cursor_read::<DexPrice>()?;
        self.validate_range("dex pricing", cursor, start_key, end_key, |key| decompose_key(*key).0)
            .map_err(|e| {
                tracing::error!("please run range with flag `--run-dex-pricing`");
                e
            })
    }

    fn validate_metadata_and_cex(&self, start_block: u64, end_block: u64) -> eyre::Result<bool> {
        let tx = self.0.ro_tx()?;

        let cex_cur = tx.new_cursor::<CexPrice>()?;
        let meta_cur = tx.new_cursor::<BlockInfo>()?;

        let (cex_pass, meta_pass) = rayon::join(
            || self.validate_range("cex pricing", cex_cur, start_block, end_block, |b| *b),
            || self.validate_range("metadata", meta_cur, start_block, end_block, |b| *b),
        );
        let (cex_pass, meta_pass) = (cex_pass?, meta_pass?);

        return Ok(cex_pass && meta_pass)
    }

    fn validate_range<T: CompressedTable>(
        &self,
        table_name: &str,
        mut cursor: CompressedCursor<T, RO>,
        start_key: T::Key,
        end_key: T::Key,
        decode_key: impl Fn(&T::Key) -> u64,
    ) -> eyre::Result<bool>
    where
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        T::Key: Clone,
    {
        let range = decode_key(&end_key) - decode_key(&start_key);
        let mut res = true;
        let mut missing = Vec::new();
        let mut cur_block = decode_key(&start_key);
        let cur = cursor.walk_range(start_key.clone()..=end_key.clone())?;
        let mut peek_cur = cur.peekable();
        if peek_cur.peek().is_none() {
            tracing::error!("missing entire block range for table {}", table_name);
            return Err(eyre::eyre!("no data for entire range"))
        }

        // because for some ranges, not every item is for a block. so we only
        // increment our counter if we know its a block update
        let mut last_updated_block = decode_key(&peek_cur.peek().unwrap().as_ref().unwrap().0) - 1;

        for entry in peek_cur {
            if cur_block % 1000 == 0 {
                tracing::info!(
                    "{} validation {:.2}% completed",
                    table_name,
                    (cur_block - decode_key(&start_key)) as f64 / (range as f64) * 100.0
                );
            }

            if let Ok(field) = entry {
                let key = decode_key(&field.0);
                while key > cur_block {
                    missing.push(cur_block);
                    res = false;
                    cur_block += 1;
                }

                // need todo this due to dex pricing
                if key != last_updated_block {
                    cur_block += 1;
                    last_updated_block = key;
                }
            } else {
                missing.push(cur_block);
                res = false;
                tracing::error!("error on db entry");
                break
            }
        }

        if cur_block - 1 != decode_key(&end_key) {
            res = false
        }

        if !res {
            // put into block ranges so printout is less spammy.
            let mut i = 0usize;
            let mut ranges = vec![vec![]];
            let mut prev = 0;

            for mb in missing {
                // new range
                let prev_block = if prev == 0 { mb } else { prev + 1 };

                if prev_block != mb {
                    if i != 0 {
                        i += 1;
                    }
                    let entry = vec![mb];
                    ranges.push(entry);
                // extend prev range
                } else {
                    ranges.get_mut(i).unwrap().push(mb);
                }
                prev = mb;
            }

            let mut missing_ranges = ranges
                .into_iter()
                .filter_map(|range| {
                    let start = range.first()?;
                    let end = range.last()?;
                    Some(format!("{}-{}", start, end))
                })
                .fold(String::new(), |acc, x| acc + "\n" + &x);

            if cur_block - 1 != decode_key(&end_key) {
                missing_ranges += &format!("\n{}-{}", cur_block - 1, decode_key(&end_key));
            }

            tracing::error!("missing {} for blocks: {}", table_name, missing_ranges);
        }
        Ok(res)
    }
}

impl LibmdbxReader for LibmdbxReadWriter {
    fn load_trace(&self, block_num: u64) -> eyre::Result<Option<Vec<TxTrace>>> {
        let tx = self.0.ro_tx()?;
        Ok(tx.get::<TxTraces>(block_num)?.and_then(|i| i.traces))
    }

    fn get_protocol(&self, address: Address) -> eyre::Result<Option<Protocol>> {
        let tx = self.0.ro_tx()?;
        Ok(tx.get::<AddressToProtocol>(address)?)
    }

    fn get_metadata_no_dex_price(&self, block_num: u64) -> eyre::Result<Metadata> {
        let block_meta = self.fetch_block_metadata(block_num)?;
        let cex_quotes = self.fetch_cex_quotes(block_num)?;
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
        let cex_quotes = self.fetch_cex_quotes(block_num)?;
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

    fn try_get_token_info(&self, address: Address) -> eyre::Result<Option<TokenInfoWithAddress>> {
        let tx = self.0.ro_tx()?;
        Ok(tx
            .get::<TokenDecimals>(address)?
            .map(|inner| TokenInfoWithAddress { inner, address }))
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
                let Some(protocol) = tx.get::<AddressToProtocol>(addr)? else {
                    continue;
                };
                let Some(info) = tx.get::<AddressToTokens>(addr)? else {
                    continue;
                };
                map.insert((addr, protocol), Pair(info.token0, info.token1));
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
                let Some(protocol) = tx.get::<AddressToProtocol>(addr)? else {
                    continue;
                };
                let Some(info) = tx.get::<AddressToTokens>(addr)? else {
                    continue;
                };
                map.entry(block).or_insert(vec![]).push((
                    addr,
                    protocol,
                    Pair(info.token0, info.token1),
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
            .into_iter()
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

    fn get_protocol_tokens(&self, address: Address) -> eyre::Result<Option<PoolTokens>> {
        Ok(self.0.ro_tx()?.get::<AddressToTokens>(address)?)
    }
}

impl LibmdbxWriter for LibmdbxReadWriter {
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
        self.0
            .write_table::<AddressToProtocol, AddressToProtocolData>(&vec![
                AddressToProtocolData::new(address, classifier_name),
            ])?;

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

        self.0
            .write_table::<AddressToTokens, AddressToTokensData>(&vec![
                AddressToTokensData::new(
                    address,
                    PoolTokens {
                        token0: tokens[0],
                        token1: tokens[1],
                        init_block: block,
                        ..Default::default()
                    },
                ),
            ])?;

        Ok(())
    }

    fn save_traces(&self, block: u64, traces: Vec<TxTrace>) -> eyre::Result<()> {
        let table = TxTracesData::new(block, TxTracesInner { traces: Some(traces) });

        Ok(self.0.write_table(&vec![table])?)
    }
}

impl LibmdbxReadWriter {
    fn fetch_block_metadata(&self, block_num: u64) -> eyre::Result<BlockMetadataInner> {
        let tx = self.0.ro_tx()?;
        tx.get::<BlockInfo>(block_num)?
            .ok_or_else(|| eyre::Report::from(reth_db::DatabaseError::Read(-1)))
    }

    fn fetch_cex_quotes(&self, block_num: u64) -> eyre::Result<CexPriceMap> {
        let tx = self.0.ro_tx()?;
        Ok(CexPriceMap(
            tx.get::<CexPrice>(block_num)?
                .ok_or_else(|| eyre::Report::from(reth_db::DatabaseError::Read(-1)))?
                .0,
        ))
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
