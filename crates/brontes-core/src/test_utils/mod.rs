#[cfg(feature = "local-reth")]
use std::sync::OnceLock;
use std::{collections::hash_map::Entry, env, fs::OpenOptions, io::Write, sync::Arc};

use alloy_consensus::Header;
use alloy_primitives::{Address, BlockHash, B256};
#[cfg(feature = "local-clickhouse")]
use brontes_database::clickhouse::Clickhouse;
#[cfg(not(feature = "local-clickhouse"))]
use brontes_database::clickhouse::ClickhouseHttpClient;
pub use brontes_database::libmdbx::{DBWriter, LibmdbxReadWriter, LibmdbxReader};
use brontes_database::{
    libmdbx::LibmdbxInit, AddressToProtocolInfo, PoolCreationBlocks, Tables, TokenDecimals,
};
use brontes_metrics::ParserMetricEvents;
use brontes_types::{
    constants::USDT_ADDRESS,
    db::{
        cex::trades::{window_loader::CexWindow, CexTradeMap},
        metadata::Metadata,
    },
    init_thread_pools,
    structured_trace::TxTrace,
    traits::TracingProvider,
    FastHashMap,
};
use futures::future::join_all;
use indicatif::MultiProgress;
#[cfg(feature = "local-reth")]
use reth_db::DatabaseEnv;
use reth_provider::ProviderError;
#[cfg(feature = "local-reth")]
use reth_tracing_ext::init_db;
#[cfg(feature = "local-reth")]
use reth_tracing_ext::TracingClient;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{
    runtime::Handle,
    sync::{
        mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
        OnceCell,
    },
};
use tracing::Level;
use tracing_subscriber::filter::Directive;

use crate::decoding::parser::TraceParser;
#[cfg(not(feature = "local-reth"))]
use crate::local_provider::LocalProvider;

const WINDOW_TIME_SEC: usize = 20;
/// Functionality to load all state needed for any testing requirements
pub struct TraceLoader {
    pub libmdbx: &'static LibmdbxReadWriter,
    pub tracing_provider: TraceParser<Box<dyn TracingProvider>, LibmdbxReadWriter>,
    // store so when we trace we don't get a closed rx error
    _metrics: UnboundedReceiver<ParserMetricEvents>,
}

impl TraceLoader {
    pub async fn new() -> Self {
        let handle = tokio::runtime::Handle::current();
        init_thread_pools(32);
        let libmdbx = get_db_handle(handle.clone()).await;

        let (a, b) = unbounded_channel();
        let tracing_provider = init_trace_parser(handle, a, libmdbx, 10).await;

        Self { libmdbx, tracing_provider, _metrics: b }
    }

    pub fn get_provider(&self) -> Arc<Box<dyn TracingProvider>> {
        self.tracing_provider.get_tracer()
    }

    pub async fn trace_block(
        &self,
        block: u64,
    ) -> Result<(BlockHash, Vec<TxTrace>, Header), TraceLoaderError> {
        if let Some(traces) = self.tracing_provider.clone().execute_block(block).await {
            Ok(traces)
        } else {
            self.fetch_missing_traces(block).await.unwrap();
            self.tracing_provider
                .clone()
                .execute_block(block)
                .await
                .ok_or_else(|| TraceLoaderError::BlockTraceError(block))
        }
    }

    pub async fn get_metadata(
        &self,
        block: u64,
        pricing: bool,
    ) -> Result<Metadata, TraceLoaderError> {
        if pricing {
            if let Ok(res) = self.test_metadata_with_pricing(block, USDT_ADDRESS) {
                Ok(res)
            } else {
                tracing::info!("test fetching missing metadata with pricing");
                self.fetch_missing_metadata(block).await?;
                self.test_metadata_with_pricing(block, USDT_ADDRESS)
                    .map_err(|_| TraceLoaderError::NoMetadataFound(block))
            }
        } else if let Ok(res) = self.test_metadata(block, USDT_ADDRESS) {
            Ok(res)
        } else {
            tracing::info!("test fetching missing metadata no pricing");
            self.fetch_missing_metadata(block).await?;
            tracing::info!("fetched missing data");
            return self
                .test_metadata(block, USDT_ADDRESS)
                .map_err(|_| TraceLoaderError::NoMetadataFound(block));
        }
    }

    pub async fn fetch_missing_traces(&self, block: u64) -> eyre::Result<()> {
        tracing::info!(%block, "fetching missing trces");

        let clickhouse = Box::leak(Box::new(load_clickhouse().await));
        let multi = MultiProgress::default();
        let tables = Arc::new(vec![(
            Tables::TxTraces,
            Tables::TxTraces.build_init_state_progress_bar(&multi, 4),
        )]);

        self.libmdbx
            .initialize_table(
                clickhouse,
                self.tracing_provider.get_tracer(),
                Tables::TxTraces,
                false,
                Some((block - 2, block + 2)),
                tables,
                false,
            )
            .await?;
        multi.clear().unwrap();

        Ok(())
    }

    pub async fn fetch_missing_metadata(&self, block: u64) -> eyre::Result<()> {
        tracing::info!(%block, "fetching missing metadata");

        let clickhouse = Box::leak(Box::new(load_clickhouse().await));
        let multi = MultiProgress::default();
        let tables = Arc::new(vec![
            (Tables::BlockInfo, Tables::BlockInfo.build_init_state_progress_bar(&multi, 4)),
            (Tables::CexPrice, Tables::CexPrice.build_init_state_progress_bar(&multi, 50)),
            (Tables::CexTrades, Tables::CexTrades.build_init_state_progress_bar(&multi, 6)),
        ]);

        futures::try_join!(
            self.libmdbx.initialize_table(
                clickhouse,
                self.tracing_provider.get_tracer(),
                Tables::BlockInfo,
                false,
                Some((block - 2, block + 2)),
                tables.clone(),
                false,
            ),
            self.libmdbx.initialize_table(
                clickhouse,
                self.tracing_provider.get_tracer(),
                Tables::CexPrice,
                false,
                Some((block - 25, block + 25)),
                tables.clone(),
                false,
            ),
            self.libmdbx.initialize_table(
                clickhouse,
                self.tracing_provider.get_tracer(),
                Tables::CexTrades,
                false,
                Some((block - 10, block + 10)),
                tables,
                false
            ),
        )?;

        multi.clear().unwrap();

        Ok(())
    }

    pub async fn fetch_missing_trades(&self, block: u64) -> eyre::Result<()> {
        tracing::info!(%block, "fetching missing metadata");

        let clickhouse = Box::leak(Box::new(load_clickhouse().await));
        let multi = MultiProgress::default();
        let tables = Arc::new(vec![(
            Tables::CexPrice,
            Tables::CexPrice.build_init_state_progress_bar(&multi, 50),
        )]);

        self.libmdbx
            .initialize_table(
                clickhouse,
                self.tracing_provider.get_tracer(),
                Tables::CexTrades,
                false,
                Some((block - 5, block + 5)),
                tables,
                false,
            )
            .await?;

        multi.clear().unwrap();
        Ok(())
    }

    pub fn test_metadata_with_pricing(
        &self,
        block_num: u64,
        quote_asset: Address,
    ) -> eyre::Result<Metadata> {
        let mut meta = self.libmdbx.get_metadata(block_num, quote_asset)?;
        meta.cex_trades = Some(self.load_cex_trades(block_num));

        Ok(meta)
    }

    pub fn test_metadata(&self, block_num: u64, quote_asset: Address) -> eyre::Result<Metadata> {
        let mut meta = self
            .libmdbx
            .get_metadata_no_dex_price(block_num, quote_asset)?;
        meta.cex_trades = Some(self.load_cex_trades(block_num));

        Ok(meta)
    }

    fn load_cex_trades(&self, block: u64) -> CexTradeMap {
        let mut cex_window = CexWindow::new(WINDOW_TIME_SEC);
        let window = cex_window.get_window_lookahead();
        // given every download is -6 + 6 around the block
        // we calculate the offset from the current block that we need
        let offsets = (window / 12) as u64;
        let mut trades = Vec::new();
        tracing::debug!(?offsets);
        for block in block - offsets..=block + offsets {
            if let Ok(res) = self.libmdbx.get_cex_trades(block) {
                trades.push(res);
            }
        }
        let last_block = block + offsets;
        cex_window.init(last_block, trades);

        cex_window.cex_trade_map()
    }

    pub async fn get_block_traces_with_header(
        &self,
        block: u64,
    ) -> Result<BlockTracesWithHeaderAnd<()>, TraceLoaderError> {
        let (_, traces, header) = self.trace_block(block).await?;
        Ok(BlockTracesWithHeaderAnd { traces, header, block, other: () })
    }

    pub async fn get_block_traces_with_header_range(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> Result<Vec<BlockTracesWithHeaderAnd<()>>, TraceLoaderError> {
        join_all((start_block..=end_block).map(|block| async move {
            let (_, traces, header) = self.trace_block(block).await?;
            Ok(BlockTracesWithHeaderAnd { traces, header, block, other: () })
        }))
        .await
        .into_iter()
        .collect()
    }

    pub async fn get_block_traces_with_header_and_metadata(
        &self,
        block: u64,
    ) -> Result<BlockTracesWithHeaderAnd<Metadata>, TraceLoaderError> {
        let (_, traces, header) = self.trace_block(block).await?;
        let metadata = self.get_metadata(block, false).await?;

        Ok(BlockTracesWithHeaderAnd { block, traces, header, other: metadata })
    }

    pub async fn get_block_traces_with_header_and_metadata_range(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> Result<Vec<BlockTracesWithHeaderAnd<Metadata>>, TraceLoaderError> {
        join_all((start_block..=end_block).map(|block| async move {
            let (_, traces, header) = self.trace_block(block).await?;
            let metadata = self.get_metadata(block, false).await?;
            Ok(BlockTracesWithHeaderAnd { traces, header, block, other: metadata })
        }))
        .await
        .into_iter()
        .collect()
    }

    pub async fn get_tx_trace_with_header(
        &self,
        tx_hash: B256,
    ) -> Result<TxTracesWithHeaderAnd<()>, TraceLoaderError> {
        let (block, tx_idx) = self
            .tracing_provider
            .get_tracer()
            .block_and_tx_index(tx_hash)
            .await?;
        let (_, traces, header) = self.trace_block(block).await?;
        let trace = traces[tx_idx].clone();

        Ok(TxTracesWithHeaderAnd { block, tx_hash, trace, header, other: () })
    }

    pub async fn get_tx_traces_with_header(
        &self,
        tx_hashes: Vec<B256>,
    ) -> Result<Vec<BlockTracesWithHeaderAnd<()>>, TraceLoaderError> {
        let mut flattened: FastHashMap<u64, BlockTracesWithHeaderAnd<()>> = FastHashMap::default();

        for res in join_all(tx_hashes.into_iter().map(|tx_hash| async move {
            let (block, tx_idx) = self
                .tracing_provider
                .get_tracer()
                .block_and_tx_index(tx_hash)
                .await?;
            let (_, traces, header) = self.trace_block(block).await?;
            let trace = traces[tx_idx].clone();

            Ok::<_, TraceLoaderError>(TxTracesWithHeaderAnd {
                block,
                tx_hash,
                trace,
                header,
                other: (),
            })
        }))
        .await
        {
            let res = res?;
            match flattened.entry(res.block) {
                Entry::Occupied(mut o) => {
                    let e = o.get_mut();
                    e.traces.push(res.trace)
                }
                Entry::Vacant(v) => {
                    let entry = BlockTracesWithHeaderAnd {
                        traces: vec![res.trace],
                        block: res.block,
                        other: (),
                        header: res.header,
                    };
                    v.insert(entry);
                }
            }
        }

        let mut res = flattened
            .into_values()
            .map(|mut traces| {
                traces
                    .traces
                    .sort_by(|t0, t1| t0.tx_index.cmp(&t1.tx_index));
                traces
            })
            .collect::<Vec<_>>();
        res.sort_by(|a, b| a.block.cmp(&b.block));

        Ok(res)
    }

    pub async fn get_tx_trace_with_header_and_metadata(
        &self,
        tx_hash: B256,
    ) -> Result<TxTracesWithHeaderAnd<Metadata>, TraceLoaderError> {
        let (block, tx_idx) = self
            .tracing_provider
            .get_tracer()
            .block_and_tx_index(tx_hash)
            .await?;
        let (_, traces, header) = self.trace_block(block).await?;
        let metadata = self.get_metadata(block, false).await?;
        let trace = traces[tx_idx].clone();

        Ok(TxTracesWithHeaderAnd { block, tx_hash, trace, header, other: metadata })
    }

    pub async fn get_tx_traces_with_header_and_metadata(
        &self,
        tx_hashes: Vec<B256>,
    ) -> Result<Vec<TxTracesWithHeaderAnd<Metadata>>, TraceLoaderError> {
        join_all(tx_hashes.into_iter().map(|tx_hash| async move {
            let (block, tx_idx) = self
                .tracing_provider
                .get_tracer()
                .block_and_tx_index(tx_hash)
                .await?;
            let (_, traces, header) = self.trace_block(block).await?;
            let metadata = self.get_metadata(block, false).await?;
            let trace = traces[tx_idx].clone();

            Ok(TxTracesWithHeaderAnd { block, tx_hash, trace, header, other: metadata })
        }))
        .await
        .into_iter()
        .collect()
    }
}

#[derive(Debug, Error)]
pub enum TraceLoaderError {
    #[error("no metadata found in libmdbx for block: {0}")]
    NoMetadataFound(u64),
    #[error("failed to trace block: {0}")]
    BlockTraceError(u64),
    #[error(transparent)]
    ProviderError(#[from] ProviderError),
    #[error(transparent)]
    EyreError(#[from] eyre::Report),
}

pub struct TxTracesWithHeaderAnd<T> {
    pub block: u64,
    pub tx_hash: B256,
    pub trace: TxTrace,
    pub header: Header,
    pub other: T,
}

pub struct BlockTracesWithHeaderAnd<T> {
    pub block: u64,
    pub traces: Vec<TxTrace>,
    pub header: Header,
    pub other: T,
}

// done because we can only have 1 instance of libmdbx or we error
static DB_HANDLE: tokio::sync::OnceCell<&'static LibmdbxReadWriter> = OnceCell::const_new();
#[cfg(feature = "local-reth")]
static RETH_DB_HANDLE: OnceLock<Arc<DatabaseEnv>> = OnceLock::new();

pub async fn get_db_handle(handle: Handle) -> &'static LibmdbxReadWriter {
    *DB_HANDLE
        .get_or_init(|| async {
            let _ = dotenv::dotenv();
            init_tracing();
            let brontes_db_path =
                env::var("BRONTES_TEST_DB_PATH").expect("No BRONTES_TEST_DB_PATH in .env");

            let this = &*Box::leak(Box::new(
                LibmdbxReadWriter::init_db_tests(&brontes_db_path).unwrap_or_else(|e| {
                    panic!("failed to open db path {}, err={}", brontes_db_path, e)
                }),
            ));

            let (tx, _rx) = unbounded_channel();
            let clickhouse = Box::leak(Box::new(load_clickhouse().await));
            let tracer = init_trace_parser(handle, tx, this, 5).await;
            if init_crit_tables(this) {
                tracing::info!("initting crit tables");
                this.initialize_full_range_tables(clickhouse, tracer.get_tracer(), false)
                    .await
                    .unwrap();
            } else {
                tracing::info!("skipping crit table init");
            }

            this
        })
        .await
}

/// will trigger a update if a test with a new highest block is written
/// or if any of the 3 critical tables are empty
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CritTablesCache {
    pub biggest_block: u64,
    pub tables: FastHashMap<Tables, usize>,
}

fn init_crit_tables(db: &LibmdbxReadWriter) -> bool {
    // try load table cache
    let tables =
        &[Tables::PoolCreationBlocks, Tables::AddressToProtocolInfo, Tables::TokenDecimals];

    let mut is_init = true;
    let mut map = FastHashMap::default();
    for table in tables {
        let cnt = match table {
            Tables::PoolCreationBlocks => db.get_table_entry_count::<PoolCreationBlocks>().unwrap(),
            Tables::AddressToProtocolInfo => {
                db.get_table_entry_count::<AddressToProtocolInfo>().unwrap()
            }
            Tables::TokenDecimals => db.get_table_entry_count::<TokenDecimals>().unwrap(),
            _ => unreachable!(),
        };
        is_init &= cnt != 0;
        map.insert(*table, cnt);
    }

    let write_fn = |block: u64| {
        let cache = CritTablesCache { biggest_block: block, tables: map };
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(".test_cache.json")
            .unwrap();
        let strd = serde_json::to_string(&cache).unwrap();

        write!(&mut file, "{}", strd).unwrap();
        file.flush().unwrap();
    };

    // try fetch highest block number. if there is no highest block number.
    // init crit tables and save current cache.
    let Ok(max_block) = db.get_highest_block_number() else {
        tracing::info!("no highest block found");
        write_fn(0);

        return true;
    };
    // try load file.
    let Ok(cache_data) = std::fs::read_to_string(".test_cache.json") else {
        tracing::info!("no .test_cache.json found");
        write_fn(max_block);
        return true;
    };

    let stats: CritTablesCache = serde_json::from_str(&cache_data).unwrap();
    // now that we have loaded the stats. lets update them.
    write_fn(max_block);

    // we init if stats.biggest block is < the db biggest block or we have a table
    // with zero entries
    tracing::info!(cache_block=?stats.biggest_block, ?max_block, ?is_init);
    stats.biggest_block < max_block || !is_init
}

#[cfg(feature = "local-reth")]
pub fn get_reth_db_handle() -> Arc<DatabaseEnv> {
    use std::path::Path;

    RETH_DB_HANDLE
        .get_or_init(|| {
            let mut db_path =
                Path::new(&env::var("DB_PATH").expect("No DB_PATH in .env")).to_path_buf();
            db_path.push("db");
            Arc::new(init_db(db_path).unwrap())
        })
        .clone()
}

// if we want more tracing/logging/metrics layers, build and push to this vec
// the stdout one (logging) is the only 1 we need
//
// peep the Database repo -> bin/sorella-db/src/cli.rs line 34 for example
pub fn init_tracing() {
    // all lower level logging directives include higher level ones (Trace includes
    // all, Debug includes all but Trace, ...)
    let verbosity_level = Level::INFO; // Error >= Warn >= Info >= Debug >= Trace
    let directive: Directive = format!("{verbosity_level}").parse().unwrap();
    let layers = vec![brontes_tracing::stdout(directive)];

    brontes_tracing::init(layers);
}

#[cfg(feature = "local-reth")]
pub async fn init_trace_parser(
    handle: Handle,
    metrics_tx: UnboundedSender<ParserMetricEvents>,
    libmdbx: &'static LibmdbxReadWriter,
    max_tasks: u32,
) -> TraceParser<Box<dyn TracingProvider>, LibmdbxReadWriter> {
    let executor = brontes_types::BrontesTaskManager::new(handle.clone(), true);

    let mut db_path =
        std::path::Path::new(&env::var("DB_PATH").expect("No DB_PATH in .env")).to_path_buf();
    let mut static_files = db_path.clone();

    db_path.push("db");
    static_files.push("static_files");

    let client = TracingClient::new_with_db(
        get_reth_db_handle(),
        max_tasks as u64,
        executor.executor(),
        static_files,
    );
    handle.spawn(executor);
    let tracer = Box::new(client) as Box<dyn TracingProvider>;

    TraceParser::new(libmdbx, Arc::new(tracer), Arc::new(metrics_tx)).await
}

#[cfg(not(feature = "local-reth"))]
pub async fn init_trace_parser(
    _handle: Handle,
    metrics_tx: UnboundedSender<ParserMetricEvents>,
    libmdbx: &'static LibmdbxReadWriter,
    _max_tasks: u32,
) -> TraceParser<Box<dyn TracingProvider>, LibmdbxReadWriter> {
    let db_endpoint = env::var("RETH_ENDPOINT").expect("No db Endpoint in .env");
    let db_port = env::var("RETH_PORT").expect("No DB port.env");
    let url = format!("{db_endpoint}:{db_port}");
    let tracer = Box::new(LocalProvider::new(url, 15)) as Box<dyn TracingProvider>;

    TraceParser::new(libmdbx, Arc::new(tracer), Arc::new(metrics_tx)).await
}

#[cfg(feature = "local-clickhouse")]
pub async fn load_clickhouse() -> Clickhouse {
    Clickhouse::new_default(None).await
}

#[cfg(not(feature = "local-clickhouse"))]
pub async fn load_clickhouse() -> ClickhouseHttpClient {
    let clickhouse_api = env::var("CLICKHOUSE_API").expect("No CLICKHOUSE_API in .env");
    let clickhouse_api_key = env::var("CLICKHOUSE_API_KEY").ok();
    ClickhouseHttpClient::new(clickhouse_api, clickhouse_api_key).await
}
