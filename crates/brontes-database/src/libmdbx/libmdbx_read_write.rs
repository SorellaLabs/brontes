use std::{
    cmp::max,
    collections::BinaryHeap,
    ops::{Bound, RangeInclusive},
    path::Path,
    sync::Arc,
};

use alloy_primitives::Address;
use brontes_pricing::Protocol;
#[cfg(feature = "cex-dex-markout")]
use brontes_types::db::cex::cex_trades::CexTradeMap;
#[cfg(feature = "cex-dex-markout")]
use brontes_types::db::initialized_state::CEX_TRADES_FLAG;
use brontes_types::{
    constants::{ETH_ADDRESS, USDC_ADDRESS, USDT_ADDRESS, WETH_ADDRESS},
    db::{
        address_metadata::AddressMetadata,
        address_to_protocol_info::ProtocolInfo,
        builder::BuilderInfo,
        cex::{CexPriceMap, FeeAdjustedQuote},
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
    normalized_actions::Action,
    pair::Pair,
    structured_trace::TxTrace,
    traits::TracingProvider,
    BlockTree, FastHashMap,
};
use dashmap::DashMap;
use eyre::{eyre, ErrReport};
use futures::Future;
use indicatif::ProgressBar;
use itertools::Itertools;
use reth_db::table::{Compress, Encode};
use reth_interfaces::db::LogLevel;
use tracing::info;

use super::types::ReturnKV;
use crate::{
    clickhouse::ClickhouseHandle,
    libmdbx::{tables::*, types::LibmdbxData, Libmdbx, LibmdbxInitializer},
    CompressedTable,
};

pub trait LibmdbxInit: LibmdbxReader + DBWriter {
    /// initializes all the tables with data via the CLI
    fn initialize_tables<T: TracingProvider, CH: ClickhouseHandle>(
        &'static self,
        clickhouse: &'static CH,
        tracer: Arc<T>,
        tables: Tables,
        clear_tables: bool,
        block_range: Option<(u64, u64)>,
        progress_bar: Arc<Vec<(Tables, ProgressBar)>>,
    ) -> impl Future<Output = eyre::Result<()>> + Send;

    /// Initialize the small tables that aren't indexed by block number
    fn initialize_full_range_tables<T: TracingProvider, CH: ClickhouseHandle>(
        &'static self,
        clickhouse: &'static CH,
        tracer: Arc<T>,
    ) -> impl Future<Output = eyre::Result<()>> + Send;

    /// initializes all the tables with missing data ranges via the CLI
    fn initialize_tables_arbitrary<T: TracingProvider, CH: ClickhouseHandle>(
        &'static self,
        clickhouse: &'static CH,
        tracer: Arc<T>,
        tables: Tables,
        block_range: Vec<u64>,
        progress_bar: Arc<Vec<(Tables, ProgressBar)>>,
    ) -> impl Future<Output = eyre::Result<()>> + Send;

    fn state_to_initialize(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> eyre::Result<StateToInitialize>;
}

// how often we will append data
const CLEAR_AM: usize = 100;

pub struct MinHeapData<T> {
    pub block: u64,
    pub data:  T,
}

impl<T> PartialEq for MinHeapData<T> {
    fn eq(&self, other: &Self) -> bool {
        self.block.eq(&other.block)
    }
}
impl<T> Eq for MinHeapData<T> {}

impl<T> PartialOrd for MinHeapData<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        other.block.partial_cmp(&self.block)
    }
}

impl<T> Ord for MinHeapData<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

pub struct LibmdbxReadWriter {
    pub db:           Libmdbx,
    /// this only applies to non instant update tables. e.g DexPriceMapping,
    /// or results. If it is a new protocol it will instantly be inserted as we
    /// always want it to be available
    pub insert_queue:
        DashMap<Tables, BinaryHeap<MinHeapData<(Vec<u8>, Vec<u8>)>>, ahash::RandomState>,
}

impl Drop for LibmdbxReadWriter {
    fn drop(&mut self) {
        std::mem::take(&mut self.insert_queue)
            .into_iter()
            .for_each(|(table, values)| {
                if values.is_empty() {
                    return
                }
                match table {
                    Tables::DexPrice => {
                        self.insert_batched_data::<DexPrice>(values).unwrap();
                    }
                    Tables::CexPrice => {
                        self.insert_batched_data::<CexPrice>(values).unwrap();
                    }
                    Tables::CexTrades => {
                        self.insert_batched_data::<CexTrades>(values).unwrap();
                    }
                    Tables::MevBlocks => {
                        self.insert_batched_data::<MevBlocks>(values).unwrap();
                    }
                    Tables::TxTraces => {
                        self.insert_batched_data::<TxTraces>(values).unwrap();
                    }
                    Tables::InitializedState => {
                        self.insert_batched_data::<InitializedState>(values)
                            .unwrap();
                    }

                    table => unreachable!("{table} doesn't have batch inserts"),
                }
            });
    }
}

impl LibmdbxReadWriter {
    pub fn init_db<P: AsRef<Path>>(path: P, log_level: Option<LogLevel>) -> eyre::Result<Self> {
        Ok(Self {
            db:           Libmdbx::init_db(path, log_level)?,
            insert_queue: DashMap::default(),
        })
    }
}

impl LibmdbxInit for LibmdbxReadWriter {
    /// initializes all the tables with data via the CLI
    async fn initialize_tables<T: TracingProvider, CH: ClickhouseHandle>(
        &'static self,
        clickhouse: &'static CH,
        tracer: Arc<T>,
        tables: Tables,
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
        tables: Tables,
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

    async fn initialize_full_range_tables<T: TracingProvider, CH: ClickhouseHandle>(
        &'static self,
        clickhouse: &'static CH,
        tracer: Arc<T>,
    ) -> eyre::Result<()> {
        let initializer = LibmdbxInitializer::new(self, clickhouse, tracer);
        initializer.initialize_full_range_tables().await?;

        Ok(())
    }

    fn state_to_initialize(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> eyre::Result<StateToInitialize> {
        let blocks = end_block - start_block;
        let block_range = (blocks / 128 + 1) as usize;
        let excess = blocks % 128;

        let mut range = vec![0u128; block_range];
        // mark tables out of scope as initialized;
        range[block_range - 1] |= u128::MAX >> excess;

        let mut tables: Vec<Vec<u128>> = Tables::ALL.into_iter().map(|_| range.clone()).collect();

        let tx = self.db.ro_tx()?;
        let mut cur = tx.new_cursor::<InitializedState>()?;
        let mut peek_cur = cur.walk_range(start_block..=end_block)?.peekable();

        if peek_cur.peek().is_none() {
            tracing::info!("entire range missing");
            let mut result = FastHashMap::default();
            let tables_to_init = default_tables_to_init();
            for table in tables_to_init {
                result.insert(table, vec![start_block as usize..=end_block as usize]);
            }
            return Ok(StateToInitialize { ranges_to_init: result })
        }

        let start_block = start_block as usize;

        for next in peek_cur {
            let Ok(entry) = next else { panic!("crit db error for InitializeState table") };
            let block = entry.0 as usize;
            let state = entry.1;

            let pos = block - start_block;

            for (table, bool) in tables_to_initialize(state) {
                tables[table as u8 as usize][pos / 128] |= (bool as u8 as u128) << (127 - pos % 128)
            }
        }
        tx.commit()?;

        let wanted_tables = default_tables_to_init();

        let table_ranges = wanted_tables
            .into_iter()
            .map(|table| {
                // fetch table from vec
                let offset = table as u8 as usize;

                let table_res = unsafe {
                    let ptr = tables.as_mut_ptr();
                    let loc = ptr.add(offset);
                    loc.replace(vec![])
                };

                let mut ranges = Vec::new();
                let mut range_start_block: Option<usize> = None;

                for (i, mut range) in table_res.into_iter().enumerate() {
                    // if there are no zeros, then this chuck is fully init
                    if range.count_zeros() == 0 {
                        continue
                    }

                    let mut sft_cnt = 0;
                    // if the range starts with a zero. set the start_block if it isn't
                    if range & 1 << 127 == 0 && range_start_block.is_none() {
                        range_start_block = Some(start_block + (i * 128));
                        sft_cnt += 1;
                        // move to left once
                        range <<= sft_cnt;
                    }

                    // while we have ones to process, continue
                    while range.count_ones() != 0 && sft_cnt <= 127 {
                        let leading_zeros = range.leading_zeros();
                        if leading_zeros == 127 {
                            break
                        }
                        // if we have leading 1's, skip to the end of the leading ones
                        else if leading_zeros == 0 {
                            // we have ones next, lets take all of them, shift them away
                            // and continue processing
                            let leading_ones = range.leading_ones();
                            range <<= leading_ones;
                            sft_cnt += leading_ones;

                            // mark the start_block now that we have shifted these out
                            let block = start_block + (i * 128) + sft_cnt as usize;
                            if range_start_block.is_some() || sft_cnt >= 128 {
                                continue
                            }
                            range_start_block = Some(block);

                            continue
                        } else {
                            // take range,
                            range <<= leading_zeros;
                            sft_cnt += leading_zeros;
                        }

                        let block = start_block + (i * 128) + sft_cnt as usize;

                        if let Some(start_block) = range_start_block.take() {
                            ranges.push(start_block..=block - 1);
                        } else {
                            range_start_block = Some(block);
                        }
                    }
                }

                // needed for case the range is a multiple of u128
                if let Some(start_block) = range_start_block.take() {
                    ranges.push(start_block..=end_block as usize);
                }

                (table, ranges)
            })
            .collect::<FastHashMap<_, _>>();

        Ok(StateToInitialize { ranges_to_init: table_ranges })
    }
}

#[derive(Debug, Default)]
pub struct StateToInitialize {
    pub ranges_to_init: FastHashMap<Tables, Vec<RangeInclusive<usize>>>,
}

impl StateToInitialize {
    pub fn get_state_for_ranges(
        &self,
        start_block: usize,
        end_block: usize,
    ) -> Vec<(Tables, Vec<RangeInclusive<u64>>)> {
        self.ranges_to_init
            .iter()
            .map(|(table, ranges)| {
                (
                    *table,
                    ranges
                        .iter()
                        .filter_map(|f| {
                            let start = *f.start();
                            let end = *f.end();
                            // if start or end out of range
                            if end < start_block || start > end_block {
                                return None
                            }

                            let new_start = std::cmp::max(start_block, start) as u64;
                            let new_end = std::cmp::min(end_block, end) as u64;
                            Some(new_start..=new_end)
                        })
                        .collect_vec(),
                )
            })
            .collect_vec()
    }

    pub fn tables_with_init_count(&self) -> impl Iterator<Item = (Tables, usize)> + '_ {
        self.ranges_to_init
            .iter()
            .map(|(table, cnt)| (*table, cnt.iter().map(|r| r.end() - r.start()).sum()))
    }
}

impl LibmdbxReader for LibmdbxReadWriter {
    fn get_dex_quotes(&self, block: u64) -> eyre::Result<DexQuotes> {
        self.fetch_dex_quotes(block)
    }

    fn load_trace(&self, block_num: u64) -> eyre::Result<Vec<TxTrace>> {
        self.db.view_db(|tx| {
            tx.get::<TxTraces>(block_num)?
                .ok_or_else(|| eyre::eyre!("missing trace for block: {}", block_num))
                .map(|i| {
                    i.traces
                        .ok_or_else(|| eyre::eyre!("missing trace for block: {}", block_num))
                })?
        })
    }

    fn get_protocol_details(&self, address: Address) -> eyre::Result<ProtocolInfo> {
        self.db.view_db(|tx| {
            tx.get::<AddressToProtocolInfo>(address)?
                .ok_or_else(|| eyre::eyre!("entry for key {:?} in AddressToProtocolInfo", address))
        })
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
            max(eth_prices.price_maker.0, eth_prices.price_maker.1),
            block_meta.private_flow.into_iter().collect(),
        )
        .into_metadata(
            cex_quotes,
            None,
            None,
            #[cfg(feature = "cex-dex-markout")]
            trades,
            #[cfg(not(feature = "cex-dex-markout"))]
            None,
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
                max(eth_prices.price_maker.0, eth_prices.price_maker.1),
                block_meta.private_flow.into_iter().collect(),
            )
            .into_metadata(
                cex_quotes,
                Some(dex_quotes),
                None,
                #[cfg(feature = "cex-dex-markout")]
                trades,
                #[cfg(not(feature = "cex-dex-markout"))]
                None,
            )
        })
    }

    fn try_fetch_token_info(&self, address: Address) -> eyre::Result<TokenInfoWithAddress> {
        self.db.view_db(|tx| {
            let address = if address == ETH_ADDRESS { WETH_ADDRESS } else { address };

            tx.get::<TokenDecimals>(address)?
                .map(|inner| TokenInfoWithAddress { inner, address })
                .ok_or_else(|| eyre::eyre!("entry for key {:?} in TokenDecimals", address))
        })
    }

    fn try_fetch_searcher_eoa_info(
        &self,
        searcher_eoa: Address,
    ) -> eyre::Result<Option<SearcherInfo>> {
        self.db.view_db(|tx| {
            tx.get::<SearcherEOAs>(searcher_eoa)
                .map_err(ErrReport::from)
        })
    }

    fn try_fetch_searcher_contract_info(
        &self,
        searcher_contract: Address,
    ) -> eyre::Result<Option<SearcherInfo>> {
        self.db.view_db(|tx| {
            tx.get::<SearcherContracts>(searcher_contract)
                .map_err(ErrReport::from)
        })
    }

    fn fetch_all_searcher_eoa_info(&self) -> eyre::Result<Vec<(Address, SearcherInfo)>> {
        self.db.view_db(|tx| {
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
        })
    }

    fn fetch_all_searcher_contract_info(&self) -> eyre::Result<Vec<(Address, SearcherInfo)>> {
        self.db.view_db(|tx| {
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
        })
    }

    fn protocols_created_before(
        &self,
        block_num: u64,
    ) -> eyre::Result<FastHashMap<(Address, Protocol), Pair>> {
        self.db.view_db(|tx| {
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
        })
    }

    fn protocols_created_range(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> eyre::Result<FastHashMap<u64, Vec<(Address, Protocol, Pair)>>> {
        self.db.view_db(|tx| {
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
        })
    }

    fn try_fetch_address_metadata(
        &self,
        address: Address,
    ) -> eyre::Result<Option<AddressMetadata>> {
        self.db
            .view_db(|tx| tx.get::<AddressMeta>(address).map_err(ErrReport::from))
    }

    fn try_fetch_builder_info(
        &self,
        builder_coinbase_addr: Address,
    ) -> eyre::Result<Option<BuilderInfo>> {
        self.db.view_db(|tx| {
            tx.get::<Builder>(builder_coinbase_addr)
                .map_err(ErrReport::from)
        })
    }

    fn fetch_all_builder_info(&self) -> eyre::Result<Vec<(Address, BuilderInfo)>> {
        self.db.view_db(|tx| {
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
        })
    }

    fn try_fetch_mev_blocks(
        &self,
        start_block: Option<u64>,
        end_block: u64,
    ) -> eyre::Result<Vec<MevBlockWithClassified>> {
        self.db.view_db(|tx| {
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
        })
    }

    fn fetch_all_mev_blocks(
        &self,
        start_block: Option<u64>,
    ) -> eyre::Result<Vec<MevBlockWithClassified>> {
        self.db.view_db(|tx| {
            let mut cursor = tx.cursor_read::<MevBlocks>()?;

            let mut res = Vec::new();

            // Start the walk from the first key-value pair
            let walker = cursor.walk(start_block)?;

            // Iterate over all the key-value pairs using the walker
            for row in walker {
                res.push(row?.1);
            }

            Ok(res)
        })
    }

    fn fetch_all_address_metadata(&self) -> eyre::Result<Vec<(Address, AddressMetadata)>> {
        self.db.view_db(|tx| {
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
        })
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
        self.db
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
        self.db
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

        self.db
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
        let data =
            MevBlocksData::new(block_number, MevBlockWithClassified { block, mev }).into_key_val();
        let (key, value) = Self::convert_into_save_bytes(data);

        let mut entry = self.insert_queue.entry(Tables::MevBlocks).or_default();
        entry.push(MinHeapData { block: block_number, data: (key.to_vec(), value) });

        if entry.len() > CLEAR_AM {
            self.insert_batched_data::<MevBlocks>(std::mem::take(&mut entry))?;
        }

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

            let mut entry = self.insert_queue.entry(Tables::DexPrice).or_default();

            quotes
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
                .for_each(|data| {
                    let data = data.into_key_val();
                    let (key, value) = Self::convert_into_save_bytes(data);
                    entry.push(MinHeapData { block: block_num, data: (key.to_vec(), value) });
                });

            // assume 150 entries per block
            if entry.len() > CLEAR_AM * 150 {
                self.insert_batched_data::<DexPrice>(std::mem::take(&mut entry))?;
            }
        }

        Ok(())
    }

    async fn write_token_info(
        &self,
        address: Address,
        decimals: u8,
        symbol: String,
    ) -> eyre::Result<()> {
        self.db
            .write_table::<TokenDecimals, TokenDecimalsData>(&[TokenDecimalsData::new(
                address,
                TokenInfo::new(decimals, symbol),
            )])
            .expect("libmdbx write failure");
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
        self.db
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
        self.db.view_db(|tx| {
            let mut addrs = tx
                .get::<PoolCreationBlocks>(block)
                .expect("libmdbx write failure")
                .map(|i| i.0)
                .unwrap_or_default();

            addrs.push(address);
            self.db
                .write_table::<PoolCreationBlocks, PoolCreationBlocksData>(&[
                    PoolCreationBlocksData::new(block, PoolsToAddresses(addrs)),
                ])
                .expect("libmdbx write failure");

            Ok(())
        })
    }

    async fn save_traces(&self, block: u64, traces: Vec<TxTrace>) -> eyre::Result<()> {
        let data = TxTracesData::new(block, TxTracesInner { traces: Some(traces) }).into_key_val();
        let (key, value) = Self::convert_into_save_bytes(data);

        let mut entry = self.insert_queue.entry(Tables::TxTraces).or_default();
        entry.push(MinHeapData { block, data: (key.to_vec(), value) });

        if entry.len() > CLEAR_AM {
            self.insert_batched_data::<TxTraces>(std::mem::take(&mut entry))?;
        }

        self.init_state_updating(block, TRACE_FLAG)
    }

    async fn write_builder_info(
        &self,
        builder_address: Address,
        builder_info: BuilderInfo,
    ) -> eyre::Result<()> {
        let data = BuilderData::new(builder_address, builder_info);
        self.db
            .write_table::<Builder, BuilderData>(&[data])
            .expect("libmdbx write failure");
        Ok(())
    }

    /// only for internal functionality (i.e. clickhouse)
    async fn insert_tree(&self, _tree: Arc<BlockTree<Action>>) -> eyre::Result<()> {
        Ok(())
    }
}

impl LibmdbxReadWriter {
    pub fn flush_init_data(&self) -> eyre::Result<()> {
        self.insert_queue.alter_all(|table, mut res| {
            tracing::info!("alter table");
            match table {
                Tables::DexPrice => {
                    let values = std::mem::take(&mut res);
                    if let Err(e) = self.insert_batched_data::<DexPrice>(values) {
                        tracing::error!(error=%e);
                    }
                }
                Tables::CexPrice => {
                    let values = std::mem::take(&mut res);
                    if let Err(e) = self.insert_batched_data::<CexPrice>(values) {
                        tracing::error!(error=%e);
                    }
                }
                Tables::CexTrades => {
                    let values = std::mem::take(&mut res);
                    if let Err(e) = self.insert_batched_data::<CexTrades>(values) {
                        tracing::error!(error=%e);
                    }
                }
                Tables::MevBlocks => {
                    let values = std::mem::take(&mut res);
                    if let Err(e) = self.insert_batched_data::<MevBlocks>(values) {
                        tracing::error!(error=%e);
                    }
                }
                Tables::TxTraces => {
                    let values = std::mem::take(&mut res);
                    if let Err(e) = self.insert_batched_data::<TxTraces>(values) {
                        tracing::error!(error=%e);
                    }
                }
                Tables::InitializedState => {
                    let values = std::mem::take(&mut res);
                    if let Err(e) = self.insert_batched_data::<InitializedState>(values) {
                        tracing::error!(error=%e);
                    }
                }
                table => tracing::error!("{table} doesn't have batch inserts"),
            }

            res
        });
        Ok(())
    }

    fn insert_batched_data<T: CompressedTable>(
        &self,
        mut data: BinaryHeap<MinHeapData<(Vec<u8>, Vec<u8>)>>,
    ) -> eyre::Result<()>
    where
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    {
        // get collections of continuous batches. append if we have a set of operations,
        // otherwise just put.
        let mut current: Vec<MinHeapData<(Vec<u8>, Vec<u8>)>> = Vec::new();
        while let Some(next) = data.pop() {
            let block = next.block;
            if let Some(last) = current.last() {
                if last.block + 1 != block {
                    let tx = self.db.rw_tx()?;
                    for buffered_entry in std::mem::take(&mut current) {
                        let (key, value) = buffered_entry.data;
                        tx.append_bytes::<T>(&key, value)?;
                    }
                    tx.commit()?;
                    current.push(next);

                    continue
                }
                // next in seq, push to buf
                current.push(next);
            } else {
                current.push(next);
            }
        }

        let rem = std::mem::take(&mut current);
        if !rem.is_empty() {
            let tx = self.db.rw_tx()?;
            for buffered_entry in rem {
                let (key, value) = buffered_entry.data;
                tx.append_bytes::<T>(&key, value)?;
            }
            tx.commit()?;
        }
        Ok(())
    }

    pub fn convert_into_save_bytes<T: CompressedTable>(
        data: ReturnKV<T>,
    ) -> (<T::Key as Encode>::Encoded, <T::Value as Compress>::Compressed)
    where
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    {
        let key = data.key.encode();
        let value: T::Value = data.value.into();
        (key, value.compress())
    }

    fn init_state_updating(&self, block: u64, flag: u8) -> eyre::Result<()> {
        self.db.view_db(|tx| {
            let mut state = tx.get::<InitializedState>(block)?.unwrap_or_default();
            state.set(flag);
            let data = InitializedStateData::new(block, state).into_key_val();

            let (key, value) = Self::convert_into_save_bytes(data);

            let mut entry = self
                .insert_queue
                .entry(Tables::InitializedState)
                .or_default();
            entry.push(MinHeapData { block, data: (key.to_vec(), value) });

            if entry.len() > CLEAR_AM * 300 {
                self.insert_batched_data::<InitializedState>(std::mem::take(&mut entry))?;
            }

            Ok(())
        })
    }

    pub fn inited_range(&self, range: RangeInclusive<u64>, flag: u8) -> eyre::Result<()> {
        let tx = self.db.ro_tx()?;
        let mut range_cursor = tx.cursor_read::<InitializedState>()?;
        let mut entry = self
            .insert_queue
            .entry(Tables::InitializedState)
            .or_default();

        for block in range {
            if let Some(mut state) = range_cursor.seek_exact(block)? {
                state.1.set(flag);
                let data = InitializedStateData::new(block, state.1).into_key_val();
                let (key, value) = Self::convert_into_save_bytes(data);
                entry.push(MinHeapData { block, data: (key.to_vec(), value) });
            } else {
                let mut init_state = InitializedStateMeta::default();
                init_state.set(flag);
                let data = InitializedStateData::from((block, init_state)).into_key_val();

                let (key, value) = Self::convert_into_save_bytes(data);
                entry.push(MinHeapData { block, data: (key.to_vec(), value) });
            }
        }
        tx.commit()?;

        if entry.len() > CLEAR_AM * 300 {
            self.insert_batched_data::<InitializedState>(std::mem::take(&mut entry))?;
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
        self.db.view_db(|tx| {
            let res = tx.get::<BlockInfo>(block_num)?.ok_or_else(|| {
                eyre!("Failed to fetch Metadata's block info for block {}", block_num)
            });

            if res.is_err() {
                self.init_state_updating(block_num, SKIP_FLAG)?;
            }

            res
        })
    }

    #[cfg(feature = "cex-dex-markout")]
    fn fetch_trades(&self, block_num: u64) -> eyre::Result<CexTradeMap> {
        self.db.view_db(|tx| {
            tx.get::<CexTrades>(block_num)?
                .ok_or_else(|| eyre!("Failed to fetch cex trades's for block {}", block_num))
        })
    }

    fn fetch_cex_quotes(&self, block_num: u64) -> eyre::Result<CexPriceMap> {
        self.db.view_db(|tx| {
            let res = tx.get::<CexPrice>(block_num)?.unwrap_or_default().0;
            Ok(CexPriceMap(res))
        })
    }

    pub fn fetch_dex_quotes(&self, block_num: u64) -> eyre::Result<DexQuotes> {
        let mut dex_quotes: Vec<Option<FastHashMap<Pair, DexPrices>>> = Vec::new();
        let (start_range, end_range) = make_filter_key_range(block_num);
        self.db.view_db(|tx| {
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
        })
    }
}

pub fn determine_eth_prices(cex_quotes: &CexPriceMap) -> FeeAdjustedQuote {
    if let Some(eth_usdt) = cex_quotes.get_binance_quote(&Pair(WETH_ADDRESS, USDT_ADDRESS)) {
        eth_usdt
    } else {
        cex_quotes
            .get_binance_quote(&Pair(WETH_ADDRESS, USDC_ADDRESS))
            .unwrap_or_default()
    }
}

fn default_tables_to_init() -> Vec<Tables> {
    let mut tables_to_init = vec![Tables::BlockInfo, Tables::DexPrice];
    #[cfg(not(feature = "local-reth"))]
    tables_to_init.push(Tables::TxTraces);
    #[cfg(not(feature = "cex-dex-markout"))]
    tables_to_init.push(Tables::CexPrice);
    #[cfg(feature = "cex-dex-markout")]
    tables_to_init.push(Tables::CexTrades);

    tables_to_init
}
pub fn tables_to_initialize(data: InitializedStateMeta) -> Vec<(Tables, bool)> {
    if data.should_ignore() {
        default_tables_to_init()
            .into_iter()
            .map(|t| (t, true))
            .collect_vec()
    } else {
        let mut tables = vec![
            (Tables::BlockInfo, data.is_initialized(META_FLAG)),
            (Tables::DexPrice, data.is_initialized(DEX_PRICE_FLAG)),
        ];

        #[cfg(not(feature = "local-reth"))]
        tables.push((Tables::TxTraces, data.is_initialized(TRACE_FLAG)));
        #[cfg(not(feature = "cex-dex-markout"))]
        tables.push((Tables::CexPrice, data.is_initialized(CEX_QUOTES_FLAG)));
        #[cfg(feature = "cex-dex-markout")]
        tables.push((Tables::CexTrades, data.is_initialized(CEX_TRADES_FLAG)));

        tables
    }
}
