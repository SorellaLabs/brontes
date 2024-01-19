use std::{
    env,
    path::{Path, PathBuf},
    sync::Arc,
};

use brontes_database::Metadata;
use brontes_database_libmdbx::Libmdbx;
use brontes_metrics::PoirotMetricEvents;
use brontes_types::{structured_trace::TxTrace, traits::TracingProvider};
use futures::future::join_all;
use log::Level;
use reth_primitives::{Header, B256};
use reth_provider::ProviderError;
use reth_tracing_ext::TracingClient;
use thiserror::Error;
use tokio::{
    runtime::Handle,
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
};
use tracing_subscriber::filter::Directive;

use crate::decoding::parser::TraceParser;

/// Functionality to load all state needed for any testing requirements
pub struct TraceLoader {
    pub libmdbx:          &'static Libmdbx,
    pub tracing_provider: TraceParser<'static, Box<dyn TracingProvider>>,
    // store so when we trace we don't get a closed rx error
    _metrics:             UnboundedReceiver<PoirotMetricEvents>,
}

impl TraceLoader {
    pub fn new() -> Self {
        let _ = dotenv::dotenv();
        init_tracing();

        let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
        let libmdbx = Box::leak(Box::new(Libmdbx::init_db(brontes_db_endpoint, None).unwrap()));

        let (a, b) = unbounded_channel();
        let tracing_provider = init_trace_parser(tokio::runtime::Handle::current(), a, libmdbx, 10);
        Self { libmdbx, tracing_provider, _metrics: b }
    }

    pub fn get_provider(&self) -> Arc<Box<dyn TracingProvider>> {
        self.tracing_provider.get_tracer()
    }

    async fn trace_block(&self, block: u64) -> Result<(Vec<TxTrace>, Header), TraceLoaderError> {
        self.tracing_provider
            .execute_block(block)
            .await
            .ok_or_else(|| TraceLoaderError::BlockTraceError(block))
    }

    async fn get_metadata(&self, block: u64) -> Result<Metadata, TraceLoaderError> {
        self.libmdbx
            .get_metadata(block)
            .map_err(|_| TraceLoaderError::NoMetadataFound(block))
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
        let metadata = self.get_metadata(block).await?;

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
                    let metadata = self.get_metadata(block).await?;
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
    ) -> Result<Vec<TxTracesWithHeaderAnd<()>>, TraceLoaderError> {
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
        .collect()
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
        let metadata = self.get_metadata(block).await?;
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
            let metadata = self.get_metadata(block).await?;
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

// if we want more tracing/logging/metrics layers, build and push to this vec
// the stdout one (logging) is the only 1 we need
// peep the Database repo -> bin/sorella-db/src/cli.rs line 34 for example
fn init_tracing() {
    // all lower level logging directives include higher level ones (Trace includes
    // all, Debug includes all but Trace, ...)
    let verbosity_level = Level::Info; // Error >= Warn >= Info >= Debug >= Trace
    let directive: Directive = format!("{verbosity_level}").parse().unwrap();
    let layers = vec![brontes_tracing::stdout(directive)];

    /*
        make sure the first field of the macro is: 'target: "brontes"',
        otherwise you will get logs from other crates (it's OD annoying trust).

        if you really want tracing from other external crates:
            replace -> let directive: Directive = format!("brontes={verbosity_level}").parse().unwrap();
            with -> let directive: Directive = format!("{verbosity_level}").parse().unwrap();

        to use the logging in a test:
        error!(target: "brontes", ...)
        warn!(target: "brontes", ...)
        info!(target: "brontes", ...)
        debug!(target: "brontes", ...)
        trace!(target: "brontes", ...)
    */

    brontes_tracing::init(layers);
}

fn init_trace_parser<'a>(
    handle: Handle,
    metrics_tx: UnboundedSender<PoirotMetricEvents>,
    libmdbx: &'a Libmdbx,
    max_tasks: u32,
) -> TraceParser<'a, Box<dyn TracingProvider>> {
    let db_path = env::var("DB_PATH").expect("No DB_PATH in .env");

    #[cfg(feature = "local")]
    let tracer = {
        let db_endpoint = env::var("RETH_ENDPOINT").expect("No db Endpoint in .env");
        let db_port = env::var("RETH_PORT").expect("No DB port.env");
        let url = format!("{db_endpoint}:{db_port}");
        Box::new(alloy_providers::provider::Provider::new(&url).unwrap())
            as Box<dyn TracingProvider>
    };

    #[cfg(not(feature = "local"))]
    let tracer = {
        let (t_handle, client) =
            TracingClient::new(Path::new(&db_path), handle.clone(), max_tasks as u64);
        handle.spawn(t_handle);

        Box::new(client) as Box<dyn TracingProvider>
    };

    let call = Box::new(|_: &_, _: &_| true);

    TraceParser::new(libmdbx, call, Arc::new(tracer), Arc::new(metrics_tx))
}

pub async fn store_traces_for_block(block_number: u64) {
    let tracer = TraceLoader::new();

    let BlockTracesWithHeaderAnd { traces, header, .. } = tracer
        .get_block_traces_with_header(block_number)
        .await
        .unwrap();

    let file = PathBuf::from(format!(
        "./crates/brontes-core/src/test_utils/liquidation_traces/{}.json",
        block_number
    ));
    let stringified = serde_json::to_string(&(traces, header)).unwrap();
    std::fs::write(&file, stringified).unwrap();
}

#[allow(unused)]
fn load_traces_for_block(block_number: u64) -> (Vec<TxTrace>, Header) {
    let file = PathBuf::from(format!(
        "./crates/brontes-core/src/test_utils/liquidation_traces/{}.json",
        block_number
    ));

    serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap()
}
