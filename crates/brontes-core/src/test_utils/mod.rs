use std::{
    collections::{hash_map::Entry, HashMap},
    env,
    path::Path,
    sync::{Arc, OnceLock},
};

pub use brontes_database::libmdbx::{LibmdbxReadWriter, LibmdbxReader, LibmdbxWriter};
use brontes_database::{clickhouse::Clickhouse, Tables};
use brontes_metrics::PoirotMetricEvents;
use brontes_types::{db::metadata::Metadata, structured_trace::TxTrace, traits::TracingProvider};
use futures::future::join_all;
use reth_primitives::{Header, B256};
use reth_provider::ProviderError;
use reth_tasks::TaskManager;
use reth_tracing_ext::TracingClient;
use thiserror::Error;
use tokio::{
    runtime::Handle,
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
};
use tracing::Level;
use tracing_subscriber::filter::Directive;

use crate::decoding::parser::TraceParser;
#[cfg(feature = "local")]
use crate::local_provider::LocalProvider;

/// Functionality to load all state needed for any testing requirements
pub struct TraceLoader {
    pub libmdbx:          &'static LibmdbxReadWriter,
    pub tracing_provider: TraceParser<'static, Box<dyn TracingProvider>, LibmdbxReadWriter>,
    // store so when we trace we don't get a closed rx error
    _metrics:             UnboundedReceiver<PoirotMetricEvents>,
}

impl TraceLoader {
    pub fn new() -> Self {
        let libmdbx = get_db_handle();
        let (a, b) = unbounded_channel();
        let tracing_provider = init_trace_parser(tokio::runtime::Handle::current(), a, libmdbx, 10);
        Self { libmdbx, tracing_provider, _metrics: b }
    }

    pub fn new_with_rt(handle: Handle) -> Self {
        let libmdbx = get_db_handle();
        let (a, b) = unbounded_channel();
        let tracing_provider = init_trace_parser(handle, a, libmdbx, 10);
        Self { libmdbx, tracing_provider, _metrics: b }
    }

    pub fn get_provider(&self) -> Arc<Box<dyn TracingProvider>> {
        self.tracing_provider.get_tracer()
    }

    pub async fn trace_block(
        &self,
        block: u64,
    ) -> Result<(Vec<TxTrace>, Header), TraceLoaderError> {
        self.tracing_provider
            .execute_block(block)
            .await
            .ok_or_else(|| TraceLoaderError::BlockTraceError(block))
    }

    pub async fn get_metadata(
        &self,
        block: u64,
        pricing: bool,
    ) -> Result<Metadata, TraceLoaderError> {
        if pricing {
            if let Ok(res) = self.test_metadata_with_pricing(block) {
                return Ok(res)
            } else {
                self.fetch_missing_metadata(block).await?;
                return self
                    .test_metadata_with_pricing(block)
                    .map_err(|_| TraceLoaderError::NoMetadataFound(block))
            }
        } else {
            if let Ok(res) = self.test_metadata(block) {
                return Ok(res)
            } else {
                self.fetch_missing_metadata(block).await?;
                return self
                    .test_metadata(block)
                    .map_err(|_| TraceLoaderError::NoMetadataFound(block))
            }
        }
    }

    pub async fn fetch_missing_metadata(&self, block: u64) -> eyre::Result<()> {
        tracing::info!(%block, "fetching missing metadata");

        let clickhouse = Arc::new(Clickhouse::default());
        self.libmdbx
            .initialize_tables(
                clickhouse.clone(),
                self.tracing_provider.get_tracer(),
                &[Tables::BlockInfo, Tables::CexPrice],
                false,
                Some((block - 2, block + 2)),
            )
            .await?;

        self.libmdbx
            .initialize_tables(
                clickhouse,
                self.tracing_provider.get_tracer(),
                &[
                    Tables::PoolCreationBlocks,
                    Tables::TokenDecimals,
                    Tables::AddressToTokens,
                    Tables::AddressToProtocol,
                ],
                false,
                None,
            )
            .await?;

        Ok(())
    }

    pub fn test_metadata_with_pricing(&self, block_num: u64) -> eyre::Result<Metadata> {
        self.libmdbx.get_metadata(block_num)
    }

    pub fn test_metadata(&self, block_num: u64) -> eyre::Result<Metadata> {
        self.libmdbx.get_metadata_no_dex_price(block_num)
    }

    pub async fn get_block_traces_with_header(
        &self,
        block: u64,
    ) -> Result<BlockTracesWithHeaderAnd<()>, TraceLoaderError> {
        let (traces, header) = self.trace_block(block).await?;
        Ok(BlockTracesWithHeaderAnd { traces, header, block, other: () })
    }

    pub async fn get_block_traces_with_header_range(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> Result<Vec<BlockTracesWithHeaderAnd<()>>, TraceLoaderError> {
        join_all(
            (start_block..=end_block)
                .into_iter()
                .map(|block| async move {
                    let (traces, header) = self.trace_block(block).await?;
                    Ok(BlockTracesWithHeaderAnd { traces, header, block, other: () })
                }),
        )
        .await
        .into_iter()
        .collect()
    }

    pub async fn get_block_traces_with_header_and_metadata(
        &self,
        block: u64,
    ) -> Result<BlockTracesWithHeaderAnd<Metadata>, TraceLoaderError> {
        let (traces, header) = self.trace_block(block).await?;
        let metadata = self.get_metadata(block, false).await?;

        Ok(BlockTracesWithHeaderAnd { block, traces, header, other: metadata })
    }

    pub async fn get_block_traces_with_header_and_metadata_range(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> Result<Vec<BlockTracesWithHeaderAnd<Metadata>>, TraceLoaderError> {
        join_all(
            (start_block..=end_block)
                .into_iter()
                .map(|block| async move {
                    let (traces, header) = self.trace_block(block).await?;
                    let metadata = self.get_metadata(block, false).await?;
                    Ok(BlockTracesWithHeaderAnd { traces, header, block, other: metadata })
                }),
        )
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
        let (traces, header) = self.trace_block(block).await?;
        let trace = traces[tx_idx].clone();

        Ok(TxTracesWithHeaderAnd { block, tx_hash, trace, header, other: () })
    }

    pub async fn get_tx_traces_with_header(
        &self,
        tx_hashes: Vec<B256>,
    ) -> Result<Vec<BlockTracesWithHeaderAnd<()>>, TraceLoaderError> {
        let mut flattened: HashMap<u64, BlockTracesWithHeaderAnd<()>> = HashMap::new();
        join_all(tx_hashes.into_iter().map(|tx_hash| async move {
            let (block, tx_idx) = self
                .tracing_provider
                .get_tracer()
                .block_and_tx_index(tx_hash)
                .await?;
            let (traces, header) = self.trace_block(block).await?;
            let trace = traces[tx_idx].clone();

            Ok(TxTracesWithHeaderAnd { block, tx_hash, trace, header, other: () })
        }))
        .await
        .into_iter()
        .for_each(|res: Result<TxTracesWithHeaderAnd<()>, TraceLoaderError>| {
            if let Ok(res) = res {
                match flattened.entry(res.block) {
                    Entry::Occupied(mut o) => {
                        let e = o.get_mut();
                        e.traces.push(res.trace)
                    }
                    Entry::Vacant(v) => {
                        let entry = BlockTracesWithHeaderAnd {
                            traces: vec![res.trace],
                            block:  res.block,
                            other:  (),
                            header: res.header,
                        };
                        v.insert(entry);
                    }
                }
            }
        });

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
        let (traces, header) = self.trace_block(block).await?;
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
            let (traces, header) = self.trace_block(block).await?;
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
    pub block:   u64,
    pub tx_hash: B256,
    pub trace:   TxTrace,
    pub header:  Header,
    pub other:   T,
}

pub struct BlockTracesWithHeaderAnd<T> {
    pub block:  u64,
    pub traces: Vec<TxTrace>,
    pub header: Header,
    pub other:  T,
}

// done because we can only have 1 instance of libmdbx or we error
static DB_HANDLE: OnceLock<LibmdbxReadWriter> = OnceLock::new();

fn get_db_handle() -> &'static LibmdbxReadWriter {
    DB_HANDLE.get_or_init(|| {
        let _ = dotenv::dotenv();
        init_tracing();
        let brontes_db_endpoint =
            env::var("BRONTES_TEST_DB_PATH").expect("No BRONTES_DB_PATH in .env");
        LibmdbxReadWriter::init_db(&brontes_db_endpoint, None)
            .expect(&format!("failed to open db path {}", brontes_db_endpoint))
    })
}

// if we want more tracing/logging/metrics layers, build and push to this vec
// the stdout one (logging) is the only 1 we need
// peep the Database repo -> bin/sorella-db/src/cli.rs line 34 for example
fn init_tracing() {
    // all lower level logging directives include higher level ones (Trace includes
    // all, Debug includes all but Trace, ...)
    let verbosity_level = Level::INFO; // Error >= Warn >= Info >= Debug >= Trace
    let directive: Directive = format!("{verbosity_level}").parse().unwrap();
    let layers = vec![brontes_tracing::stdout(directive)];

    brontes_tracing::init(layers);
}

fn init_trace_parser<'a>(
    handle: Handle,
    metrics_tx: UnboundedSender<PoirotMetricEvents>,
    libmdbx: &'a LibmdbxReadWriter,
    max_tasks: u32,
) -> TraceParser<'a, Box<dyn TracingProvider>, LibmdbxReadWriter> {
    let db_path = env::var("DB_PATH").expect("No DB_PATH in .env");

    #[cfg(feature = "local")]
    let tracer = {
        let db_endpoint = env::var("RETH_ENDPOINT").expect("No db Endpoint in .env");
        let db_port = env::var("RETH_PORT").expect("No DB port.env");
        let url = format!("{db_endpoint}:{db_port}");
        Box::new(LocalProvider::new(url)) as Box<dyn TracingProvider>
    };
    #[cfg(not(feature = "local"))]
    let tracer = {
        let executor = TaskManager::new(handle.clone());
        let client = TracingClient::new(Path::new(&db_path), max_tasks as u64, executor.executor());
        handle.spawn(executor);
        Box::new(client) as Box<dyn TracingProvider>
    };

    let call = Box::new(|_: &_, _: &_| true);

    TraceParser::new(libmdbx, call, Arc::new(tracer), Arc::new(metrics_tx))
}
