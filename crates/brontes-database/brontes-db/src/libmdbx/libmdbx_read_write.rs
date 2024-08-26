use std::{ops::RangeInclusive, path::Path, sync::Arc};

use alloy_primitives::Address;
use brontes_metrics::db_reads::LibmdbxMetrics;
use brontes_pricing::Protocol;
use brontes_types::{
    constants::{ETH_ADDRESS, WETH_ADDRESS},
    db::{
        address_metadata::AddressMetadata,
        address_to_protocol_info::ProtocolInfo,
        builder::BuilderInfo,
        cex::{quotes::CexPriceMap, trades::CexTradeMap},
        dex::{make_filter_key_range, DexPrices, DexQuotes},
        initialized_state::{
            InitializedStateMeta, CEX_QUOTES_FLAG, CEX_TRADES_FLAG, DATA_NOT_PRESENT_NOT_AVAILABLE,
            DATA_PRESENT, DEX_PRICE_FLAG, META_FLAG,
        },
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
use malachite::Rational;
use reth_db::table::{Compress, Encode};
use reth_interfaces::db::LogLevel;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tracing::{info, instrument};

use super::{
    libmdbx_writer::{LibmdbxWriter, StampedWriterMessage, WriterMessage},
    types::ReturnKV,
    ReadWriteCache,
};
#[cfg(feature = "local-clickhouse")]
use crate::clickhouse::ClickhouseCritTableCount;
use crate::{
    clickhouse::ClickhouseHandle,
    libmdbx::{tables::*, types::LibmdbxData, Libmdbx, LibmdbxInitializer},
    CompressedTable,
};

pub trait LibmdbxInit: LibmdbxReader + DBWriter {
    /// initializes all the tables with data via the CLI
    fn initialize_table<T: TracingProvider, CH: ClickhouseHandle>(
        &'static self,
        clickhouse: &'static CH,
        tracer: Arc<T>,
        tables: Tables,
        clear_tables: bool,
        block_range: Option<(u64, u64)>,
        progress_bar: Arc<Vec<(Tables, ProgressBar)>>,
        metrics: bool,
    ) -> impl Future<Output = eyre::Result<()>> + Send;

    /// Initialize the small tables that aren't indexed by block number
    fn initialize_full_range_tables<T: TracingProvider, CH: ClickhouseHandle>(
        &'static self,
        clickhouse: &'static CH,
        tracer: Arc<T>,
        metrics: bool,
    ) -> impl Future<Output = eyre::Result<()>> + Send;

    /// initializes all the tables with missing data ranges via the CLI
    fn initialize_table_arbitrary<T: TracingProvider, CH: ClickhouseHandle>(
        &'static self,
        clickhouse: &'static CH,
        tracer: Arc<T>,
        tables: Tables,
        block_range: Vec<u64>,
        progress_bar: Arc<Vec<(Tables, ProgressBar)>>,
        metrics: bool,
    ) -> impl Future<Output = eyre::Result<()>> + Send;

    fn state_to_initialize(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> eyre::Result<StateToInitialize>;

    fn get_db_range(&self) -> eyre::Result<(u64, u64)>;
}

#[derive(Clone)]
pub struct LibmdbxReadWriter {
    pub db:  Arc<Libmdbx>,
    pub tx:  UnboundedSender<StampedWriterMessage>,
    metrics: Option<LibmdbxMetrics>,
    // 100 shards for now, might change in future
    cache:   ReadWriteCache,
}

impl LibmdbxReadWriter {
    pub fn init_db<P: AsRef<Path>>(
        path: P,
        log_level: Option<LogLevel>,
        ex: &BrontesTaskExecutor,
        metrics: bool,
    ) -> eyre::Result<Self> {
        // 5 gb total
        let memory_per_table_mb = 1_000;
        let (tx, rx) = unbounded_channel();
        let yapper = UnboundedYapperReceiver::new(rx, 1500, "libmdbx write channel".to_string());
        let db = Arc::new(Libmdbx::init_db(path, log_level)?);
        let shutdown = ex.get_graceful_shutdown();

        // start writing task on own thread
        let writer = LibmdbxWriter::new(db.clone(), yapper, metrics);
        writer.run(shutdown);

        Ok(Self {
            db,
            tx,
            metrics: metrics.then(LibmdbxMetrics::default),
            cache: ReadWriteCache::new(memory_per_table_mb, metrics),
        })
    }

    pub fn init_db_tests<P: AsRef<Path>>(path: P) -> eyre::Result<Self> {
        // 5 gb total
        let memory_per_table_mb = 1_000;
        let (tx, rx) = unbounded_channel();
        let yapper = UnboundedYapperReceiver::new(rx, 1500, "libmdbx write channel".to_string());
        let db = Arc::new(Libmdbx::init_db(path, None)?);

        // start writing task on own thread
        let writer = LibmdbxWriter::new(db.clone(), yapper, false);
        writer.run_no_shutdown();

        Ok(Self { db, tx, metrics: None, cache: ReadWriteCache::new(memory_per_table_mb, false) })
    }
}

impl LibmdbxInit for LibmdbxReadWriter {
    /// Initializes a table for a given range of blocks
    async fn initialize_table<T: TracingProvider, CH: ClickhouseHandle>(
        &'static self,
        clickhouse: &'static CH,
        tracer: Arc<T>,
        tables: Tables,
        clear_tables: bool,
        block_range: Option<(u64, u64)>, // inclusive of start only
        progress_bar: Arc<Vec<(Tables, ProgressBar)>>,
        metrics: bool,
    ) -> eyre::Result<()> {
        let initializer = LibmdbxInitializer::new(self, clickhouse, tracer, metrics);
        initializer
            .initialize(tables, clear_tables, block_range, progress_bar)
            .await?;

        Ok(())
    }

    /// Initializes a table for a given range of blocks
    async fn initialize_table_arbitrary<T: TracingProvider, CH: ClickhouseHandle>(
        &'static self,
        clickhouse: &'static CH,
        tracer: Arc<T>,
        tables: Tables,
        block_range: Vec<u64>,
        progress_bar: Arc<Vec<(Tables, ProgressBar)>>,
        metrics: bool,
    ) -> eyre::Result<()> {
        let block_range = Box::leak(Box::new(block_range));

        let initializer = LibmdbxInitializer::new(self, clickhouse, tracer, metrics);
        initializer
            .initialize_arbitrary_state(tables, block_range, progress_bar)
            .await?;

        Ok(())
    }

    async fn initialize_full_range_tables<T: TracingProvider, CH: ClickhouseHandle>(
        &'static self,
        clickhouse: &'static CH,
        tracer: Arc<T>,
        metrics: bool,
    ) -> eyre::Result<()> {
        let initializer = LibmdbxInitializer::new(self, clickhouse, tracer, metrics);
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

    fn get_db_range(&self) -> eyre::Result<(u64, u64)> {
        let tx = self.db.ro_tx()?;
        let mut cur = tx.cursor_read::<BlockInfo>()?;

        let start_block = cur
            .first()?
            .ok_or_else(|| {
                eyre::eyre!(
                    "no start block found. database most likely empty.\n run `brontes db \
                     download-snapshot <place-to-write-db>` in order to download the most recent \
                     db"
                )
            })?
            .0;

        let end_block = cur
            .last()?
            .ok_or_else(|| {
                eyre::eyre!(
                    "no end block found. database most likely empty.\n run `brontes db \
                     download-snapshot <place-to-write-db>` in order to download the most recent \
                     db"
                )
            })?
            .0;

        Ok((start_block, end_block))
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

    pub fn merge(&mut self, other: StateToInitialize) {
        for (table, ranges) in other.ranges_to_init {
            self.ranges_to_init.entry(table).or_default().extend(ranges);
        }
    }
}

impl LibmdbxReader for LibmdbxReadWriter {
    fn get_most_recent_block(&self) -> eyre::Result<u64> {
        self.get_highest_block_number()
    }

    #[brontes_macros::metrics_call(ptr=metrics,scope,db_read,"get_dex_quotes")]
    fn get_dex_quotes(&self, block: u64) -> eyre::Result<DexQuotes> {
        self.fetch_dex_quotes(block)
    }

    fn get_cex_trades(&self, block: u64) -> eyre::Result<CexTradeMap> {
        self.fetch_trades(block)
    }

    fn has_dex_quotes(&self, block_num: u64) -> eyre::Result<bool> {
        self.db.view_db(|tx| {
            let Some(state) = tx.get::<InitializedState>(block_num)? else { return Ok(false) };
            Ok(state.is_initialized(DEX_PRICE_FLAG))
        })
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
            match self
                .cache
                .protocol_info(true, |handle| handle.get(&address))
            {
                Some(Some(e)) => Ok(e.clone()),
                Some(None) => {
                    Err(eyre::eyre!("entry for key {:?} in AddressToProtocolInfo", address))
                }
                None => tx
                    .get::<AddressToProtocolInfo>(address)
                    .inspect(|data| {
                        self.cache.protocol_info(false, |lock| {
                            lock.get_with(address, || data.clone());
                        })
                    })?
                    .ok_or_else(|| {
                        eyre::eyre!("entry for key {:?} in AddressToProtocolInfo", address)
                    }),
            }
        })
    }

    #[brontes_macros::metrics_call(ptr=metrics, scope, db_read,"metadata_no_dex_price")]
    fn get_metadata_no_dex_price(
        &self,
        block_num: u64,
        quote_asset: Address,
    ) -> eyre::Result<Metadata> {
        let block_meta = self.fetch_block_metadata(block_num)?;
        let cex_quotes = self.fetch_cex_quotes(block_num)?;

        let eth_price =
            determine_eth_prices(&cex_quotes, block_meta.block_timestamp * 1_000_000, quote_asset);

        Ok(BlockMetadata::new(
            block_num,
            block_meta.block_hash,
            block_meta.block_timestamp,
            block_meta.relay_timestamp,
            block_meta.p2p_timestamp,
            block_meta.proposer_fee_recipient,
            block_meta.proposer_mev_reward,
            eth_price.unwrap_or_default(),
            block_meta.private_flow.into_iter().collect(),
        )
        .into_metadata(cex_quotes, None, None, None))
    }

    #[brontes_macros::metrics_call(ptr=metrics,scope,db_read,"metadata")]
    fn get_metadata(&self, block_num: u64, quote_asset: Address) -> eyre::Result<Metadata> {
        let block_meta = self.fetch_block_metadata(block_num)?;
        let cex_quotes = self.fetch_cex_quotes(block_num)?;
        let dex_quotes = self.fetch_dex_quotes(block_num)?;

        let eth_price =
            determine_eth_prices(&cex_quotes, block_meta.block_timestamp * 1_000_000, quote_asset);

        Ok({
            BlockMetadata::new(
                block_num,
                block_meta.block_hash,
                block_meta.block_timestamp,
                block_meta.relay_timestamp,
                block_meta.p2p_timestamp,
                block_meta.proposer_fee_recipient,
                block_meta.proposer_mev_reward,
                eth_price.unwrap_or_default(),
                block_meta.private_flow.into_iter().collect(),
            )
            .into_metadata(cex_quotes, Some(dex_quotes), None, None)
        })
    }

    #[brontes_macros::metrics_call(ptr=metrics,scope, db_read, "try_fetch_token_info")]
    fn try_fetch_token_info(&self, og_address: Address) -> eyre::Result<TokenInfoWithAddress> {
        let address = if og_address == ETH_ADDRESS { WETH_ADDRESS } else { og_address };

        self.db
            .view_db(|tx| match self.cache.token_info(true, |lock| lock.get(&address)) {
                Some(Some(e)) => {
                    let mut info = TokenInfoWithAddress { inner: e, address: og_address };
                    if og_address == ETH_ADDRESS {
                        info.symbol = "ETH".to_string();
                    }
                    Ok(info)
                }
                Some(None) => Err(eyre::eyre!("entry for key {:?} in TokenDecimals", address)),
                None => tx
                    .get::<TokenDecimals>(address)
                    .inspect(|data| {
                        self.cache.token_info(false, |lock| {
                            lock.get_with(address, || data.clone());
                        })
                    })?
                    .map(|inner| TokenInfoWithAddress { inner, address: og_address })
                    .map(|mut inner| {
                        // quick patch
                        if og_address == ETH_ADDRESS {
                            inner.symbol = "ETH".to_string();
                            inner
                        } else {
                            inner
                        }
                    })
                    .ok_or_else(|| eyre::eyre!("entry for key {:?} in TokenDecimals", address)),
            })
    }

    #[brontes_macros::metrics_call(ptr=metrics,scope,db_read,"try_fetch_searcher_eoa_infos")]
    fn try_fetch_searcher_eoa_infos(
        &self,
        searcher_eoa: Vec<Address>,
    ) -> eyre::Result<FastHashMap<Address, SearcherInfo>> {
        self.db.view_db(|tx| {
            let mut res = FastHashMap::default();
            for eoa in searcher_eoa {
                match self.cache.searcher_eoa(true, |h| h.get(&eoa)) {
                    Some(Some(val)) => {
                        res.insert(eoa, val.clone());
                    }
                    Some(None) => continue,
                    None => {
                        if let Ok(Some(r)) = tx
                            .get::<SearcherEOAs>(eoa)
                            .map_err(ErrReport::from)
                            .inspect(|data| {
                                self.cache.searcher_eoa(false, |f| {
                                    f.get_with(eoa, || data.clone());
                                })
                            })
                        {
                            res.insert(eoa, r);
                        }
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
        match self.cache.searcher_eoa(true, |f| f.get(&searcher_eoa)) {
            Some(Some(e)) => return Ok(Some(e.clone())),
            Some(None) => return Ok(None),
            None => self
                .db
                .view_db(|tx| {
                    tx.get::<SearcherEOAs>(searcher_eoa)
                        .map_err(ErrReport::from)
                })
                .inspect(|data| {
                    self.cache.searcher_eoa(false, |f| {
                        f.get_with(searcher_eoa, || data.clone());
                    });
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
            for contract in searcher {
                match self.cache.searcher_contract(true, |h| h.get(&contract)) {
                    Some(Some(val)) => {
                        res.insert(contract, val.clone());
                    }
                    Some(None) => continue,
                    None => {
                        if let Ok(Some(r)) = tx
                            .get::<SearcherContracts>(contract)
                            .map_err(ErrReport::from)
                            .inspect(|data| {
                                self.cache.searcher_contract(false, |f| {
                                    f.get_with(contract, || data.clone());
                                })
                            })
                        {
                            res.insert(contract, r);
                        }
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
        match self
            .cache
            .searcher_contract(true, |f| f.get(&searcher_contract))
        {
            Some(Some(e)) => return Ok(Some(e.clone())),
            Some(None) => return Ok(None),
            None => self
                .db
                .view_db(|tx| {
                    tx.get::<SearcherContracts>(searcher_contract)
                        .map_err(ErrReport::from)
                })
                .inspect(|data| {
                    self.cache.searcher_contract(false, |f| {
                        f.get_with(searcher_contract, || data.clone());
                    });
                }),
        }
    }

    #[instrument(level = "error", skip_all)]
    fn fetch_all_searcher_eoa_info(&self) -> eyre::Result<Vec<(Address, SearcherInfo)>> {
        self.db.export_db(
            None,
            |start_key, tx| {
                let mut cur = tx.cursor_read::<SearcherEOAs>()?;
                if let Some(key) = start_key {
                    let _ = cur.seek(key);
                } else {
                    // move to first entry and make sure .next() is first
                    let _ = cur.first();
                    let _ = cur.prev();
                }
                Ok(cur)
            },
            |cursor| Ok(cursor.next().map(|inner| inner.map(|i| (i.0, i.1)))?),
        )
    }

    #[instrument(level = "error", skip_all)]
    fn fetch_all_searcher_contract_info(&self) -> eyre::Result<Vec<(Address, SearcherInfo)>> {
        self.db.export_db(
            None,
            |start_key, tx| {
                let mut cur = tx.cursor_read::<SearcherContracts>()?;
                if let Some(key) = start_key {
                    let _ = cur.seek(key);
                } else {
                    // move to first entry and make sure .next() is first
                    let _ = cur.first();
                    let _ = cur.prev();
                }
                Ok(cur)
            },
            |cursor| Ok(cursor.next().map(|inner| inner.map(|i| (i.0, i.1)))?),
        )
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
            for addr in addresses {
                match self.cache.address_meta(true, |h| h.get(&addr)) {
                    Some(Some(val)) => {
                        res.insert(addr, val.clone());
                    }
                    Some(None) => continue,
                    None => {
                        if let Ok(Some(r)) = tx
                            .get::<AddressMeta>(addr)
                            .map_err(ErrReport::from)
                            .inspect(|data| {
                                self.cache.address_meta(false, |f| {
                                    f.get_with(addr, || data.clone());
                                })
                            })
                        {
                            res.insert(addr, r);
                        }
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
        match self.cache.address_meta(true, |f| f.get(&address)) {
            Some(Some(e)) => return Ok(Some(e.clone())),
            Some(None) => return Ok(None),
            None => self
                .db
                .view_db(|tx| tx.get::<AddressMeta>(address).map_err(ErrReport::from))
                .inspect(|data| {
                    self.cache.address_meta(false, |f| {
                        f.get_with(address, || data.clone());
                    });
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

    #[instrument(level = "error", skip_all)]
    fn fetch_all_builder_info(&self) -> eyre::Result<Vec<(Address, BuilderInfo)>> {
        self.db.export_db(
            None,
            |start_key, tx| {
                let mut cur = tx.cursor_read::<Builder>()?;
                if let Some(key) = start_key {
                    let _ = cur.seek(key);
                } else {
                    // move to first entry and make sure .next() is first
                    let _ = cur.first();
                    let _ = cur.prev();
                }
                Ok(cur)
            },
            |cursor| Ok(cursor.next().map(|inner| inner.map(|i| (i.0, i.1)))?),
        )
    }

    #[instrument(level = "error", skip_all)]
    fn try_fetch_mev_blocks(
        &self,
        start_block: Option<u64>,
        end_block: u64,
    ) -> eyre::Result<Vec<MevBlockWithClassified>> {
        self.db.export_db(
            start_block,
            |start_key, tx| {
                let mut cur = tx.cursor_read::<MevBlocks>()?;
                if let Some(key) = start_key {
                    let _ = cur.seek(key);
                } else {
                    // move to first entry and make sure .next() is first
                    let _ = cur.first();
                    let _ = cur.prev();
                }
                Ok(cur)
            },
            |cursor| {
                Ok(cursor
                    .next()
                    .map(|inner| inner.filter(|f| f.0 <= end_block).map(|i| i.1))?)
            },
        )
    }

    #[instrument(level = "error", skip_all)]
    fn fetch_all_mev_blocks(
        &self,
        start_block: Option<u64>,
    ) -> eyre::Result<Vec<MevBlockWithClassified>> {
        self.db.export_db(
            start_block,
            |start_key, tx| {
                let mut cur = tx.cursor_read::<MevBlocks>()?;
                if let Some(key) = start_key {
                    let _ = cur.seek(key);
                } else {
                    // move to first entry and make sure .next() is first
                    let _ = cur.first();
                    // let _ = cur.prev();
                }
                Ok(cur)
            },
            |cursor| Ok(cursor.next().map(|inner| inner.map(|i| i.1))?),
        )
    }

    #[instrument(level = "error", skip_all)]
    fn fetch_all_address_metadata(&self) -> eyre::Result<Vec<(Address, AddressMetadata)>> {
        self.db.export_db(
            None,
            |start_key, tx| {
                let mut cur = tx.cursor_read::<AddressMeta>()?;
                if let Some(key) = start_key {
                    let _ = cur.seek(key);
                } else {
                    // move to first entry and make sure .next() is first
                    let _ = cur.first();
                    let _ = cur.prev();
                }
                Ok(cur)
            },
            |cursor| Ok(cursor.next().map(|inner| inner.map(|i| (i.0, i.1)))?),
        )
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
        self.cache.searcher_eoa(false, |handle| {
            handle.insert(eoa_address, Some(eoa_info.clone()));
        });

        if let (Some(addr), Some(info)) = (contract_address, &contract_info) {
            self.cache.searcher_contract(false, |handle| {
                handle.insert(addr, Some(info.clone()));
            });
        }

        Ok(self.tx.send(
            WriterMessage::SearcherInfo {
                eoa_address,
                contract_address,
                eoa_info: Box::new(eoa_info),
                contract_info: Box::new(contract_info),
            }
            .stamp(),
        )?)
    }

    async fn write_searcher_eoa_info(
        &self,
        searcher_eoa: Address,
        searcher_info: SearcherInfo,
    ) -> eyre::Result<()> {
        self.cache.searcher_eoa(false, |handle| {
            handle.insert(searcher_eoa, Some(searcher_info.clone()));
        });

        Ok(self.tx.send(
            WriterMessage::SearcherEoaInfo { searcher_eoa, searcher_info: Box::new(searcher_info) }
                .stamp(),
        )?)
    }

    async fn write_searcher_contract_info(
        &self,
        searcher_contract: Address,
        searcher_info: SearcherInfo,
    ) -> eyre::Result<()> {
        self.cache.searcher_contract(false, |handle| {
            handle.insert(searcher_contract, Some(searcher_info.clone()));
        });

        Ok(self.tx.send(
            WriterMessage::SearcherContractInfo {
                searcher_contract,
                searcher_info: Box::new(searcher_info),
            }
            .stamp(),
        )?)
    }

    async fn write_address_meta(
        &self,
        address: Address,
        metadata: AddressMetadata,
    ) -> eyre::Result<()> {
        self.cache.address_meta(false, |handle| {
            handle.insert(address, Some(metadata.clone()));
        });

        Ok(self
            .tx
            .send(WriterMessage::AddressMeta { address, metadata: Box::new(metadata) }.stamp())?)
    }

    async fn save_mev_blocks(
        &self,
        block_number: u64,
        block: MevBlock,
        mev: Vec<Bundle>,
    ) -> eyre::Result<()> {
        Ok(self
            .tx
            .send(WriterMessage::MevBlocks { block_number, block: Box::new(block), mev }.stamp())?)
    }

    async fn write_dex_quotes(
        &self,
        block_number: u64,
        quotes: Option<DexQuotes>,
    ) -> eyre::Result<()> {
        Ok(self
            .tx
            .send(WriterMessage::DexQuotes { block_number, quotes }.stamp())?)
    }

    async fn write_token_info(
        &self,
        address: Address,
        decimals: u8,
        symbol: String,
    ) -> eyre::Result<()> {
        self.cache.token_info(false, |handle| {
            let token_info = TokenInfo::new(decimals, symbol.clone());
            handle.insert(address, Some(token_info.clone()));
        });

        Ok(self
            .tx
            .send(WriterMessage::TokenInfo { address, decimals, symbol }.stamp())?)
    }

    async fn insert_pool(
        &self,
        block: u64,
        address: Address,
        tokens: &[Address],
        curve_lp_token: Option<Address>,
        classifier_name: Protocol,
    ) -> eyre::Result<()> {
        self.cache.protocol_info(false, |handle| {
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
            handle.insert(address, Some(details.clone()));
        });

        Ok(self.tx.send(
            WriterMessage::Pool {
                block,
                address,
                tokens: tokens.to_vec(),
                curve_lp_token,
                classifier_name,
            }
            .stamp(),
        )?)
    }

    async fn save_traces(&self, block: u64, traces: Vec<TxTrace>) -> eyre::Result<()> {
        Ok(self
            .tx
            .send(WriterMessage::Traces { block, traces }.stamp())?)
    }

    async fn write_builder_info(
        &self,
        builder_address: Address,
        builder_info: BuilderInfo,
    ) -> eyre::Result<()> {
        Ok(self.tx.send(
            WriterMessage::BuilderInfo { builder_address, builder_info: Box::new(builder_info) }
                .stamp(),
        )?)
    }

    /// only for internal functionality (i.e. clickhouse)
    async fn insert_tree(&self, _tree: BlockTree<Action>) -> eyre::Result<()> {
        Ok(())
    }

    /// only for internal functionality (i.e. clickhouse)
    async fn write_block_analysis(
        &self,
        _: brontes_types::db::block_analysis::BlockAnalysis,
    ) -> eyre::Result<()> {
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

    #[cfg(feature = "local-clickhouse")]
    pub fn get_crit_table_count(&self) -> eyre::Result<ClickhouseCritTableCount> {
        let pool_creation = self.get_table_entry_count::<PoolCreationBlocks>()? as u64;
        let address_to_protocol = self.get_table_entry_count::<AddressToProtocolInfo>()? as u64;
        let tokens = self.get_table_entry_count::<TokenDecimals>()? as u64;
        let builder = self.get_table_entry_count::<Builder>()? as u64;
        let address_meta = self.get_table_entry_count::<AddressMeta>()? as u64;

        Ok(ClickhouseCritTableCount {
            pool_creation,
            address_to_protocol,
            tokens,
            builder,
            address_meta,
        })
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
    fn init_state_updating(&self, block: u64, flag: u16, availability: u16) -> eyre::Result<()> {
        self.db.view_db(|tx| {
            let mut state = tx.get::<InitializedState>(block)?.unwrap_or_default();
            state.set(flag, availability);
            let data = InitializedStateData::new(block, state);
            self.db.write_table(&[data])?;

            Ok(())
        })
    }

    pub fn inited_range_arbitrary(
        &self,
        range: impl Iterator<Item = u64>,
        flag: u16,
    ) -> eyre::Result<Vec<InitializedStateData>> {
        self.db.view_db(|tx| {
            let mut res = Vec::new();
            for block in range {
                let mut state = tx.get::<InitializedState>(block)?.unwrap_or_default();
                state.set(flag, DATA_PRESENT);
                res.push(InitializedStateData::new(block, state));
            }
            Ok(res)
        })
    }

    pub fn inited_range_items(
        &self,
        range: RangeInclusive<u64>,
        flag: u16,
    ) -> eyre::Result<Vec<InitializedStateData>> {
        let tx = self.db.ro_tx()?;
        let mut range_cursor = tx.cursor_read::<InitializedState>()?;
        let mut res = Vec::new();

        for block in range {
            if let Some(mut state) = range_cursor.seek_exact(block)? {
                state.1.set(flag, DATA_PRESENT);
                res.push(InitializedStateData::new(block, state.1));
            } else {
                let mut init_state = InitializedStateMeta::default();
                init_state.set(flag, DATA_PRESENT);
                res.push(InitializedStateData::from((block, init_state)));
            }
        }

        tx.commit()?;
        Ok(res)
    }

    pub fn inited_range(&self, range: RangeInclusive<u64>, flag: u16) -> eyre::Result<()> {
        let tx = self.db.ro_tx()?;
        let mut range_cursor = tx.cursor_read::<InitializedState>()?;
        let mut entry = Vec::new();

        for block in range {
            if let Some(mut state) = range_cursor.seek_exact(block)? {
                state.1.set(flag, DATA_PRESENT);
                let data = InitializedStateData::new(block, state.1).into_key_val();
                let (key, value) = Self::convert_into_save_bytes(data);
                entry.push((key.to_vec(), value));
            } else {
                let mut init_state = InitializedStateMeta::default();
                init_state.set(flag, DATA_PRESENT);
                let data = InitializedStateData::from((block, init_state)).into_key_val();

                let (key, value) = Self::convert_into_save_bytes(data);
                entry.push((key.to_vec(), value));
            }
        }
        tx.commit()?;

        self.insert_batched_data::<InitializedState>(entry)?;

        Ok(())
    }

    fn fetch_block_metadata(&self, block_num: u64) -> eyre::Result<BlockMetadataInner> {
        self.db.view_db(|tx| {
            tx.get::<BlockInfo>(block_num)?.ok_or_else(|| {
                let _ =
                    self.init_state_updating(block_num, META_FLAG, DATA_NOT_PRESENT_NOT_AVAILABLE);
                eyre!("Failed to fetch Metadata's block info for block {}", block_num)
            })
        })
    }

    pub fn fetch_trades(&self, block: u64) -> eyre::Result<CexTradeMap> {
        self.db.view_db(|tx| {
            tx.get::<CexTrades>(block)?
                .ok_or_else(|| eyre::eyre!("no cex trades"))
                .inspect_err(|_| {
                    let _ = self.init_state_updating(
                        block,
                        CEX_TRADES_FLAG,
                        DATA_NOT_PRESENT_NOT_AVAILABLE,
                    );
                })
        })
    }

    pub fn fetch_cex_quotes(&self, block_num: u64) -> eyre::Result<CexPriceMap> {
        self.db.view_db(|tx| {
            let res = tx.get::<CexPrice>(block_num)?.unwrap_or_else(|| {
                let _ = self.init_state_updating(
                    block_num,
                    CEX_QUOTES_FLAG,
                    DATA_NOT_PRESENT_NOT_AVAILABLE,
                );
                CexPriceMap::default()
            });

            Ok(res)
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

    pub fn send_message(&self, message: WriterMessage) -> eyre::Result<()> {
        Ok(self.tx.send(message.stamp())?)
    }

    pub fn get_table_entry_count<T>(&self) -> eyre::Result<usize>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    {
        Ok(self.db.ro_tx()?.entries::<T>()?)
    }

    pub fn get_highest_block_number(&self) -> eyre::Result<u64> {
        self.db
            .ro_tx()?
            .cursor_read::<MevBlocks>()?
            .last()?
            .map(|v| v.0)
            .ok_or_else(|| eyre::eyre!("no max block found"))
    }
}

pub fn determine_eth_prices(
    cex_quotes: &CexPriceMap,
    block_timestamp: u64,
    quote_asset: Address,
) -> Option<Rational> {
    Some(
        cex_quotes
            .get_quote_from_most_liquid_exchange(
                &Pair(quote_asset, WETH_ADDRESS),
                block_timestamp,
                None,
            )?
            .maker_taker_mid()
            .0,
    )
}

fn default_tables_to_init() -> Vec<Tables> {
    vec![Tables::BlockInfo, Tables::DexPrice, Tables::CexPrice, Tables::CexTrades]
}

pub fn tables_to_initialize(data: InitializedStateMeta) -> Vec<(Tables, bool)> {
    vec![
        (Tables::BlockInfo, data.is_initialized(META_FLAG)),
        (Tables::DexPrice, data.is_initialized(DEX_PRICE_FLAG)),
        (Tables::CexPrice, data.is_initialized(CEX_QUOTES_FLAG)),
        (Tables::CexTrades, data.is_initialized(CEX_TRADES_FLAG)),
    ]
}
