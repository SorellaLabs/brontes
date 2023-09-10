use reth_provider::BlockIdReader;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
};
use tokio::task::JoinError;

use crate::{
    executor::{Executor, TaskKind},
    init_trace,
};
use poirot_types::structured_trace::TxTrace;

use self::parser::TraceParser;
use alloy_etherscan::Client;
use ethers_core::types::Chain;
use futures::{stream::FuturesUnordered, Future};
use reth_primitives::{BlockId, BlockNumberOrTag, Header, H256};
use reth_tracing::TracingClient;

use tokio::{sync::mpsc::UnboundedSender, task::JoinHandle};

mod parser;
mod utils;
use poirot_metrics::{trace::types::TraceMetricEvent, PoirotMetricEvents};

pub(crate) const UNKNOWN: &str = "unknown";
pub(crate) const RECEIVE: &str = "receive";
pub(crate) const FALLBACK: &str = "fallback";
const CACHE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10_000);
const CACHE_DIRECTORY: &str = "./abi_cache";

pub type ParserFuture = Pin<
    Box<dyn Future<Output = Result<Option<(Vec<TxTrace>, Header)>, JoinError>> + Send + 'static>,
>;

pub struct Parser {
    executor: Executor,
    parser: Arc<TraceParser>,
}

impl Parser {
    pub fn new(
        metrics_tx: UnboundedSender<PoirotMetricEvents>,
        etherscan_key: &str,
        db_path: &str,
    ) -> Self {
        let executor = Executor::new();
        let tracer =
            Arc::new(TracingClient::new(Path::new(db_path), executor.runtime.handle().clone()));

        let etherscan_client = Client::new_cached(
            Chain::Mainnet,
            etherscan_key,
            Some(PathBuf::from(CACHE_DIRECTORY)),
            CACHE_TIMEOUT,
        )
        .unwrap();
        let parser = TraceParser::new(etherscan_client, Arc::clone(&tracer), Arc::new(metrics_tx));

        Self { executor, parser: Arc::new(parser) }
    }

    pub fn get_block_hash_for_number(
        &self,
        block_num: u64,
    ) -> reth_interfaces::Result<Option<H256>> {
        self.parser.tracer.trace.provider().block_hash_for_id(block_num.into())
    }

    /// executes the tracing of a given block
    pub fn execute(&self, block_num: u64) -> ParserFuture {
        Box::pin(self.executor.spawn_result_task_as(
            Self::execute_block(self.parser.clone(), block_num),
            TaskKind::Default,
        )) as ParserFuture
    }

    /// executes the tracing of a given block
    async fn execute_block(
        this: self::Arc<TraceParser>,
        block_num: u64,
    ) -> Option<(Vec<TxTrace>, Header)> {
        let parity_trace = this.trace_block(block_num).await;

        if parity_trace.0.is_none() {
            this.metrics_tx
                .send(TraceMetricEvent::BlockMetricRecieved(parity_trace.1).into())
                .unwrap();
            return None
        }

        let traces = this.parse_block(parity_trace.0.unwrap(), block_num).await;
        this.metrics_tx.send(TraceMetricEvent::BlockMetricRecieved(traces.1).into()).unwrap();
        Some((traces.0, traces.2))
    }
}
