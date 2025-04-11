use std::{pin::Pin, sync::Arc};

use alloy_consensus::Header;
use alloy_primitives::{BlockHash, B256};
use brontes_database::libmdbx::{DBWriter, LibmdbxReader};
use brontes_types::structured_trace::TxTrace;
pub use brontes_types::traits::TracingProvider;
use futures::Future;
use tokio::sync::mpsc::UnboundedSender;

use self::parser::TraceParser;

#[cfg(feature = "dyn-decode")]
mod dyn_decode;

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
use alloy_rpc_types::BlockId;

pub type ParserFuture =
    Pin<Box<dyn Future<Output = Option<(BlockHash, Vec<TxTrace>, Header)>> + Send + 'static>>;

pub type TraceClickhouseFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

pub struct Parser<T: TracingProvider, DB: LibmdbxReader + DBWriter> {
    parser: TraceParser<T, DB>,
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
