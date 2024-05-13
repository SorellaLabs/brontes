use std::{
    cmp::max,
    ops::{Bound, RangeInclusive},
    path::Path,
    sync::Arc,
};

use alloy_primitives::Address;
use brontes_metrics::db_reads::LibmdbxMetrics;
use brontes_pricing::Protocol;
#[cfg(feature = "cex-dex-markout")]
use brontes_types::db::cex::cex_trades::CexTradeMap;
#[cfg(not(feature = "cex-dex-markout"))]
use brontes_types::db::initialized_state::CEX_QUOTES_FLAG;
#[cfg(feature = "cex-dex-markout")]
use brontes_types::db::initialized_state::CEX_TRADES_FLAG;
use brontes_types::{
    constants::{ETH_ADDRESS, USDC_ADDRESS, USDT_ADDRESS, WETH_ADDRESS},
    db::{
        address_metadata::AddressMetadata,
        address_to_protocol_info::ProtocolInfo,
        builder::BuilderInfo,
        cex::{CexPriceMap, FeeAdjustedQuote},
        dex::{make_filter_key_range, DexPrices, DexQuotes},
        initialized_state::{InitializedStateMeta, DEX_PRICE_FLAG, META_FLAG},
        metadata::{BlockMetadata, BlockMetadataInner, Metadata},
        mev_block::MevBlockWithClassified,
        searcher::SearcherInfo,
        token_info::{TokenInfo, TokenInfoWithAddress},
        traits::{DBWriter, LibmdbxReader},
    },
    mev::{Bundle, MevBlock},
    normalized_actions::Action,
    pair::Pair,
    structured_trace::TxTrace,
    traits::TracingProvider,
    BlockTree, BrontesTaskExecutor, FastHashMap, UnboundedYapperReceiver,
};
use eyre::{eyre, ErrReport};
use futures::Future;
use indicatif::ProgressBar;
use itertools::Itertools;
use reth_db::table::{Compress, Encode};
use reth_interfaces::db::LogLevel;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tracing::{info, instrument};

use super::{
    libmdbx_writer::{LibmdbxWriter, WriterMessage},
    types::ReturnKV,
};
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

use schnellru::{ByMemoryUsage, LruMap};

pub struct LibmdbxReadWriter {
    pub db:  Arc<Libmdbx>,
    tx:      UnboundedSender<WriterMessage>,
    metrics: Option<LibmdbxMetrics>,
    cache:   ReadWriteCache,
}

const MEGABYTE: usize = 1024 * 1024;

pub struct ReadWriteCache {
    pub address_meta: std::sync::Mutex<
        LruMap<Address, Option<AddressMetadata>, ByMemoryUsage, ahash::RandomState>,
    >,
    pub searcher_eoa:
        parking_lot::Mutex<LruMap<Address, Option<SearcherInfo>, ByMemoryUsage, ahash::RandomState>>,
    pub searcher_contract:
        parking_lot::Mutex<LruMap<Address, Option<SearcherInfo>, ByMemoryUsage, ahash::RandomState>>,
    pub protocol_info:
        parking_lot::Mutex<LruMap<Address, Option<ProtocolInfo>, ByMemoryUsage, ahash::RandomState>>,
    pub token_info:
        parking_lot::Mutex<LruMap<Address, Option<TokenInfo>, ByMemoryUsage, ahash::RandomState>>,
}

impl ReadWriteCache {
    pub fn new(memory_per_table_mb: usize) -> Self {
        Self {
            address_meta:      LruMap::with_hasher(
                ByMemoryUsage::new(memory_per_table_mb * MEGABYTE),
                ahash::RandomState::new(),
            )
            .into(),
            searcher_eoa:      LruMap::with_hasher(
                ByMemoryUsage::new(memory_per_table_mb * MEGABYTE),
                ahash::RandomState::new(),
            )
            .into(),
            searcher_contract: LruMap::with_hasher(
                ByMemoryUsage::new(memory_per_table_mb * MEGABYTE),
                ahash::RandomState::new(),
            )
            .into(),
            protocol_info:     LruMap::with_hasher(
                ByMemoryUsage::new(memory_per_table_mb * MEGABYTE),
                ahash::RandomState::new(),
            )
            .into(),
            token_info:        LruMap::with_hasher(
                ByMemoryUsage::new(memory_per_table_mb * MEGABYTE),
                ahash::RandomState::new(),
            )
            .into(),
        }
    }
}

impl LibmdbxReadWriter {
    pub fn init_db<P: AsRef<Path>>(
        path: P,
        log_level: Option<LogLevel>,
        ex: &BrontesTaskExecutor,
    ) -> eyre::Result<Self> {
        let memory_per_table_mb = 100;
        let (tx, rx) = unbounded_channel();
        let yapper = UnboundedYapperReceiver::new(rx, 1500, "libmdbx write channel".to_string());
        let db = Arc::new(Libmdbx::init_db(path, log_level)?);

        // start writing task
        let writer = LibmdbxWriter::new(db.clone(), yapper);
        ex.spawn_critical_with_graceful_shutdown_signal("libmdbx writer", |shutdown| async move {
            writer.run_until_shutdown(shutdown).await
        });

        Ok(Self {
            db,
            tx,
            metrics: Some(LibmdbxMetrics::default()),
            cache: ReadWriteCache::new(memory_per_table_mb),
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
    #[brontes_macros::metrics_call(ptr=metrics,scope,db_read,"get_dex_quotes")]
    fn get_dex_quotes(&self, block: u64) -> eyre::Result<DexQuotes> {
        self.fetch_dex_quotes(block)
    }

    #[brontes_macros::metrics_call(ptr=metrics,scope,db_read,"load_trace")]
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

    #[brontes_macros::metrics_call(ptr=metrics,scope,db_read,"protocol_info")]
    fn get_protocol_details(&self, address: Address) -> eyre::Result<ProtocolInfo> {
        self.db.view_db(|tx| {
            let mut lock = self.cache.protocol_info.lock().unwrap();
            match lock.get(&address) {
                Some(Some(e)) => return Ok(e.clone()),
                Some(None) => {
                    return Err(eyre::eyre!("entry for key {:?} in AddressToProtocolInfo", address))
                }
                None => tx
                    .get::<AddressToProtocolInfo>(address)
                    .inspect(|data| {
                        lock.get_or_insert(address, || data.clone());
                    })?
                    .ok_or_else(|| {
                        eyre::eyre!("entry for key {:?} in AddressToProtocolInfo", address)
                    }),
            }
        })
    }

    #[brontes_macros::metrics_call(ptr=metrics,scope,db_read,"metadata_no_dex_price")]
    fn get_metadata_no_dex_price(&self, block_num: u64) -> eyre::Result<Metadata> {
        let block_meta = self.fetch_block_metadata(block_num)?;
        let cex_quotes = self.fetch_cex_quotes(block_num)?;
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

    #[brontes_macros::metrics_call(ptr=metrics,scope,db_read,"metadata")]
    fn get_metadata(&self, block_num: u64) -> eyre::Result<Metadata> {
        let block_meta = self.fetch_block_metadata(block_num)?;
        let cex_quotes = self.fetch_cex_quotes(block_num)?;
        let dex_quotes = self.fetch_dex_quotes(block_num)?;
        let eth_prices = determine_eth_prices(&cex_quotes);

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

    #[brontes_macros::metrics_call(ptr=metrics,scope,db_read,"try_fetch_token_info")]
    fn try_fetch_token_info(&self, address: Address) -> eyre::Result<TokenInfoWithAddress> {
        let address = if address == ETH_ADDRESS { WETH_ADDRESS } else { address };

        self.db.view_db(|tx| {
            let mut lock = self.cache.token_info.lock().unwrap();
            match lock.get(&address) {
                Some(Some(e)) => return Ok(TokenInfoWithAddress { inner: e.clone(), address }),
                Some(None) => {
                    return Err(eyre::eyre!("entry for key {:?} in TokenDecimals", address))
                }
                None => tx
                    .get::<TokenDecimals>(address)
                    .inspect(|data| {
                        lock.get_or_insert(address, || data.clone());
                    })?
                    .map(|inner| TokenInfoWithAddress { inner, address })
                    .ok_or_else(|| eyre::eyre!("entry for key {:?} in TokenDecimals", address)),
            }
        })
    }

    #[brontes_macros::metrics_call(ptr=metrics,scope,db_read,"try_fetch_searcher_eoa_infos")]
    fn try_fetch_searcher_eoa_infos(
        &self,
        searcher_eoa: Vec<Address>,
    ) -> eyre::Result<FastHashMap<Address, SearcherInfo>> {
        self.db.view_db(|tx| {
            let mut res = FastHashMap::default();
            let mut lock = self.cache.searcher_eoa.lock().unwrap();
            for eoa in searcher_eoa {
                match lock.get(&eoa) {
                    Some(Some(val)) => {
                        res.insert(eoa, val.clone());
                    }
                    Some(None) => continue,
                    None => {
                        let next = tx
                            .get::<SearcherEOAs>(eoa)
                            .map_err(ErrReport::from)
                            .inspect(|data| {
                                lock.get_or_insert(eoa, || data.clone());
                            })?;
                        let Some(next) = next else { continue };
                        res.insert(eoa, next);
                    }
                }
            }
            Ok(res)
        })
    }

    #[brontes_macros::metrics_call(ptr=metrics,scope,db_read,"try_fetch_searcher_eoa_info")]
    fn try_fetch_searcher_eoa_info(
        &self,
        searcher_eoa: Address,
    ) -> eyre::Result<Option<SearcherInfo>> {
        let mut lock = self.cache.searcher_eoa.lock().unwrap();
        match lock.get(&searcher_eoa) {
            Some(Some(e)) => return Ok(Some(e.clone())),
            Some(None) => return Ok(None),
            None => self
                .db
                .view_db(|tx| {
                    tx.get::<SearcherEOAs>(searcher_eoa)
                        .map_err(ErrReport::from)
                })
                .inspect(|data| {
                    lock.get_or_insert(searcher_eoa, || data.clone());
                }),
        }
    }

    #[brontes_macros::metrics_call(ptr=metrics,scope,db_read,"try_fetch_searcher_contract_infos")]
    fn try_fetch_searcher_contract_infos(
        &self,
        searcher: Vec<Address>,
    ) -> eyre::Result<FastHashMap<Address, SearcherInfo>> {
        self.db.view_db(|tx| {
            let mut res = FastHashMap::default();
            let mut lock = self.cache.searcher_contract.lock().unwrap();

            for eoa in searcher {
                match lock.get(&eoa) {
                    Some(Some(val)) => {
                        res.insert(eoa, val.clone());
                    }
                    Some(None) => continue,
                    None => {
                        let next = tx
                            .get::<SearcherContracts>(eoa)
                            .map_err(ErrReport::from)
                            .inspect(|data| {
                                lock.get_or_insert(eoa, || data.clone());
                            })?;
                        let Some(next) = next else { continue };
                        res.insert(eoa, next);
                    }
                }
            }
            Ok(res)
        })
    }

    #[brontes_macros::metrics_call(ptr=metrics,scope,db_read,"try_fetch_searcher_contract_info")]
    fn try_fetch_searcher_contract_info(
        &self,
        searcher_contract: Address,
    ) -> eyre::Result<Option<SearcherInfo>> {
        let mut lock = self.cache.searcher_contract.lock().unwrap();
        match lock.get(&searcher_contract) {
            Some(Some(e)) => return Ok(Some(e.clone())),
            Some(None) => return Ok(None),
            None => self
                .db
                .view_db(|tx| {
                    tx.get::<SearcherContracts>(searcher_contract)
                        .map_err(ErrReport::from)
                })
                .inspect(|data| {
                    lock.get_or_insert(searcher_contract, || data.clone());
                }),
        }
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

    #[brontes_macros::metrics_call(ptr=metrics,scope,db_read,"try_fetch_address_metadatas")]
    fn try_fetch_address_metadatas(
        &self,
        addresses: Vec<Address>,
    ) -> eyre::Result<FastHashMap<Address, AddressMetadata>> {
        self.db.view_db(|tx| {
            let mut res = FastHashMap::default();
            let mut lock = self.cache.address_meta.lock().unwrap();

            for eoa in addresses {
                match lock.get(&eoa) {
                    Some(Some(val)) => {
                        res.insert(eoa, val.clone());
                    }
                    Some(None) => continue,
                    None => {
                        let next = tx
                            .get::<AddressMeta>(eoa)
                            .map_err(ErrReport::from)
                            .inspect(|data| {
                                lock.get_or_insert(eoa, || data.clone());
                            })?;
                        let Some(next) = next else { continue };
                        res.insert(eoa, next);
                    }
                }
            }
            Ok(res)
        })
    }

    #[brontes_macros::metrics_call(ptr=metrics,scope,db_read,"try_fetch_address_metadata")]
    fn try_fetch_address_metadata(
        &self,
        address: Address,
    ) -> eyre::Result<Option<AddressMetadata>> {
        let mut lock = self.cache.address_meta.lock().unwrap();

        match lock.get(&address) {
            Some(Some(e)) => return Ok(Some(e.clone())),
            Some(None) => return Ok(None),
            None => self
                .db
                .view_db(|tx| tx.get::<AddressMeta>(address).map_err(ErrReport::from))
                .inspect(|data| {
                    lock.get_or_insert(address, || data.clone());
                }),
        }
    }

    #[brontes_macros::metrics_call(ptr=metrics,scope,db_read,"try_fetch_builder_info")]
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
        let mut lock = self.cache.searcher_eoa.lock().unwrap();
        lock.insert(eoa_address, Some(eoa_info.clone()));

        if let (Some(addr), Some(info)) = (contract_address, &contract_info) {
            let mut lock = self.cache.searcher_contract.lock().unwrap();
            lock.insert(addr, Some(info.clone()));
        }

        Ok(self.tx.send(WriterMessage::SearcherInfo {
            eoa_address,
            contract_address,
            eoa_info,
            contract_info,
        })?)
    }

    async fn write_searcher_eoa_info(
        &self,
        searcher_eoa: Address,
        searcher_info: SearcherInfo,
    ) -> eyre::Result<()> {
        let mut lock = self.cache.searcher_eoa.lock().unwrap();
        lock.insert(searcher_eoa, Some(searcher_info.clone()));

        Ok(self
            .tx
            .send(WriterMessage::SearcherEoaInfo { searcher_eoa, searcher_info })?)
    }

    async fn write_searcher_contract_info(
        &self,
        searcher_contract: Address,
        searcher_info: SearcherInfo,
    ) -> eyre::Result<()> {
        let mut lock = self.cache.searcher_contract.lock().unwrap();
        lock.insert(searcher_contract, Some(searcher_info.clone()));

        Ok(self
            .tx
            .send(WriterMessage::SearcherContractInfo { searcher_contract, searcher_info })?)
    }

    async fn write_address_meta(
        &self,
        address: Address,
        metadata: AddressMetadata,
    ) -> eyre::Result<()> {
        let mut lock = self.cache.address_meta.lock().unwrap();
        lock.insert(address, Some(metadata.clone()));

        Ok(self
            .tx
            .send(WriterMessage::AddressMeta { address, metadata })?)
    }

    async fn save_mev_blocks(
        &self,
        block_number: u64,
        block: MevBlock,
        mev: Vec<Bundle>,
    ) -> eyre::Result<()> {
        Ok(self
            .tx
            .send(WriterMessage::MevBlocks { block_number, block, mev })?)
    }

    async fn write_dex_quotes(
        &self,
        block_number: u64,
        quotes: Option<DexQuotes>,
    ) -> eyre::Result<()> {
        Ok(self
            .tx
            .send(WriterMessage::DexQuotes { block_number, quotes })?)
    }

    async fn write_token_info(
        &self,
        address: Address,
        decimals: u8,
        symbol: String,
    ) -> eyre::Result<()> {
        let token_info = TokenInfo::new(decimals, symbol.clone());
        let mut lock = self.cache.token_info.lock().unwrap();
        lock.insert(address, Some(token_info));

        Ok(self
            .tx
            .send(WriterMessage::TokenInfo { address, decimals, symbol })?)
    }

    async fn insert_pool(
        &self,
        block: u64,
        address: Address,
        tokens: &[Address],
        curve_lp_token: Option<Address>,
        classifier_name: Protocol,
    ) -> eyre::Result<()> {
        let mut tokens_i = tokens.iter();
        let default = Address::ZERO;
        let details = ProtocolInfo {
            protocol: classifier_name,
            init_block: block,
            token0: *tokens_i.next().unwrap_or(&default),
            token1: *tokens_i.next().unwrap_or(&default),
            token2: tokens_i.next().cloned(),
            token3: tokens_i.next().cloned(),
            token4: tokens_i.next().cloned(),
            curve_lp_token,
        };

        let mut lock = self.cache.protocol_info.lock().unwrap();
        lock.insert(address, Some(details));

        Ok(self.tx.send(WriterMessage::Pool {
            block,
            address,
            tokens: tokens.to_vec(),
            curve_lp_token,
            classifier_name,
        })?)
    }

    async fn save_traces(&self, block: u64, traces: Vec<TxTrace>) -> eyre::Result<()> {
        Ok(self.tx.send(WriterMessage::Traces { block, traces })?)
    }

    async fn write_builder_info(
        &self,
        builder_address: Address,
        builder_info: BuilderInfo,
    ) -> eyre::Result<()> {
        Ok(self
            .tx
            .send(WriterMessage::BuilderInfo { builder_address, builder_info })?)
    }

    /// only for internal functionality (i.e. clickhouse)
    async fn insert_tree(&self, _tree: BlockTree<Action>) -> eyre::Result<()> {
        Ok(())
    }
}

impl LibmdbxReadWriter {
    #[instrument(target = "libmdbx_read_write::insert_batched_data", skip_all, level = "warn")]
    fn insert_batched_data<T: CompressedTable>(
        &self,
        data: Vec<(Vec<u8>, Vec<u8>)>,
    ) -> eyre::Result<()>
    where
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    {
        let tx = self.db.rw_tx()?;

        for (key, value) in data {
            tx.put_bytes::<T>(&key, value)?;
        }

        tx.commit()?;
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

    #[instrument(target = "libmdbx_read_write::init_state_updating", skip_all, level = "warn")]
    fn init_state_updating(&self, block: u64, flag: u8) -> eyre::Result<()> {
        self.db.view_db(|tx| {
            let mut state = tx.get::<InitializedState>(block)?.unwrap_or_default();
            state.set(flag);
            let data = InitializedStateData::new(block, state);
            self.db.write_table(&[data])?;

            Ok(())
        })
    }

    pub fn inited_range(&self, range: RangeInclusive<u64>, flag: u8) -> eyre::Result<()> {
        let tx = self.db.ro_tx()?;
        let mut range_cursor = tx.cursor_read::<InitializedState>()?;
        let mut entry = Vec::new();

        for block in range {
            if let Some(mut state) = range_cursor.seek_exact(block)? {
                state.1.set(flag);
                let data = InitializedStateData::new(block, state.1).into_key_val();
                let (key, value) = Self::convert_into_save_bytes(data);
                entry.push((key.to_vec(), value));
            } else {
                let mut init_state = InitializedStateMeta::default();
                init_state.set(flag);
                let data = InitializedStateData::from((block, init_state)).into_key_val();

                let (key, value) = Self::convert_into_save_bytes(data);
                entry.push((key.to_vec(), value));
            }
        }
        tx.commit()?;

        self.insert_batched_data::<InitializedState>(entry)?;

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
            tx.get::<BlockInfo>(block_num)?.ok_or_else(|| {
                eyre!("Failed to fetch Metadata's block info for block {}", block_num)
            })
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
