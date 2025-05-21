use std::{collections::HashMap, pin::Pin, sync::Arc};

use alloy_rpc_types::Log;
use brontes_database::libmdbx::{DBWriter, LibmdbxReader};
pub use brontes_types::traits::{LogProvider, TracingProvider};
use brontes_types::{structured_trace::TxTrace, Protocol};
use futures::Future;
use reth_primitives::{BlockHash, BlockNumberOrTag, Header, B256};
use alloy_primitives::{Address, FixedBytes};
use tokio::sync::mpsc::UnboundedSender;

use self::{log_parser::EthLogParser, parser::TraceParser};

#[cfg(feature = "dyn-decode")]
mod dyn_decode;

pub mod log_parser;
pub mod parser;
mod utils;
use brontes_metrics::{
    range::GlobalRangeMetrics, trace::types::TraceMetricEvent, ParserMetricEvents,
};
#[allow(dead_code)]
pub(crate) const UNKNOWN: &str = "unknown";
#[allow(dead_code)]
pub(crate) const RECEIVE: &str = "receive";
#[allow(dead_code)]
pub(crate) const FALLBACK: &str = "fallback";
use reth_primitives::BlockId;

pub type LogParserFuture =
    Pin<Box<dyn Future<Output = Option<(u64, HashMap<Protocol, Vec<Log>>)>> + Send + 'static>>;

pub type ParserFuture =
    Pin<Box<dyn Future<Output = Option<(BlockHash, Vec<TxTrace>, Header)>> + Send + 'static>>;

pub type TraceClickhouseFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

pub struct Parser<T: TracingProvider, DB: LibmdbxReader + DBWriter> {
    parser: TraceParser<T, DB>,
}

pub struct LogParser<T: LogProvider, DB: LibmdbxReader + DBWriter> {
    parser: EthLogParser<T, DB>,
}

impl<T: TracingProvider, DB: LibmdbxReader + DBWriter> Parser<T, DB> {
    pub async fn new(
        metrics_tx: UnboundedSender<ParserMetricEvents>,
        libmdbx: &'static DB,
        tracing: T,
    ) -> Self {
        let parser = TraceParser::new(libmdbx, Arc::new(tracing), Arc::new(metrics_tx)).await;

        Self { parser }
    }

    #[cfg(not(feature = "local-reth"))]
    pub async fn get_latest_block_number(&self) -> eyre::Result<u64> {
        self.parser.tracer.best_block_number().await
    }

    pub fn get_tracer(&self) -> Arc<T> {
        self.parser.get_tracer()
    }

    #[cfg(feature = "local-reth")]
    pub fn get_latest_block_number(&self) -> eyre::Result<u64> {
        self.parser.tracer.best_block_number()
    }

    pub async fn get_block_hash_for_number(&self, block_num: u64) -> eyre::Result<Option<B256>> {
        self.parser.tracer.block_hash_for_id(block_num).await
    }

    /// executes the tracing of a given block
    pub fn execute(
        &self,
        block_num: u64,
        id: usize,
        metrics: Option<GlobalRangeMetrics>,
    ) -> ParserFuture {
        // This will satisfy its lifetime scope do to the lifetime itself living longer
        // than the process that runs brontes.
        let parser = self.parser.clone();

        tracing::info!(target: "brontes", "executing block: {:?}", block_num);

        if let Some(metrics) = metrics {
            Box::pin(metrics.block_tracing(id, move || Box::pin(parser.execute_block(block_num))))
                as ParserFuture
        } else {
            Box::pin(parser.execute_block(block_num)) as ParserFuture
        }
    }

    /// ensures no libmdbx write
    pub fn execute_discovery(&self, block_num: u64) -> ParserFuture {
        // This will satisfy its lifetime scope do to the lifetime itself living longer
        // than the process that runs brontes.
        let parser = self.parser.clone();

        Box::pin(parser.execute_block_discovery(block_num)) as ParserFuture
    }

    pub fn trace_for_clickhouse(&self, block_num: u64) -> TraceClickhouseFuture {
        // This will satisfy its lifetime scope do to the lifetime itself living longer
        // than the process that runs brontes.
        let parser = self.parser.clone();

        Box::pin(parser.trace_clickhouse_block(block_num)) as TraceClickhouseFuture
    }
}

impl<T: LogProvider, DB: LibmdbxReader + DBWriter> LogParser<T, DB> {
    pub async fn new(
        libmdbx: &'static DB,
        provider: Arc<T>,
        filters: HashMap<Protocol, (Address, FixedBytes<32>)>,
    ) -> Self {
        let parser = EthLogParser::new(libmdbx, provider, filters).await;
        Self { parser }
    }

    #[cfg(not(feature = "local-reth"))]
    pub async fn get_latest_block_number(&self) -> eyre::Result<u64> {
        self.parser.best_block_number().await
    }

    /// ensures no libmdbx write
    pub async fn execute_discovery(&self, start_block: u64, end_block: u64) -> eyre::Result<HashMap<Protocol, Vec<Log>>> {
        // This will satisfy its lifetime scope do to the lifetime itself living longer
        // than the process that runs brontes.
        let parser = self.parser.clone();
        parser.execute_block_discovery(start_block, end_block).await
    }
}
