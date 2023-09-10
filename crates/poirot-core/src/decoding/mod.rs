use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc
};

use alloy_etherscan::Client;
use ethers_core::types::Chain;
use futures::{stream::FuturesUnordered, Future};
use poirot_types::structured_trace::TxTrace;
use reth_primitives::{BlockId, BlockNumberOrTag, Header, H256};
use reth_provider::BlockIdReader;
use reth_tracing::TracingClient;
use tokio::{
    sync::mpsc::UnboundedSender,
    task::{JoinError, JoinHandle}
};

use self::parser::TraceParser;
use crate::{
    executor::{Executor, TaskKind},
    init_trace
};

mod parser;
mod utils;
use poirot_metrics::{trace::types::TraceMetricEvent, PoirotMetricEvents};

pub(crate) const UNKNOWN: &str = "unknown";
pub(crate) const RECEIVE: &str = "receive";
pub(crate) const FALLBACK: &str = "fallback";
const CACHE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10_000);
const CACHE_DIRECTORY: &str = "./abi_cache";

pub type ParserFuture = Pin<
    Box<dyn Future<Output = Result<Option<(Vec<TxTrace>, Header)>, JoinError>> + Send + 'static>
>;

pub struct Parser {
    executor: Executor,
    parser:   Arc<TraceParser>
}

impl Parser {
    pub fn new(
        metrics_tx: UnboundedSender<PoirotMetricEvents>,
        etherscan_key: &str,
        db_path: &str
    ) -> Self {
        let executor = Executor::new();
        let tracer =
            Arc::new(TracingClient::new(Path::new(db_path), executor.runtime.handle().clone()));

        let etherscan_client = Client::new_cached(
            Chain::Mainnet,
            etherscan_key,
            Some(PathBuf::from(CACHE_DIRECTORY)),
            CACHE_TIMEOUT
        )
        .unwrap();
        let parser = TraceParser::new(etherscan_client, Arc::clone(&tracer), Arc::new(metrics_tx));

        Self { executor, parser: Arc::new(parser) }
    }

    pub fn get_block_hash_for_number(
        &self,
        block_num: u64
    ) -> reth_interfaces::Result<Option<H256>> {
        self.parser
            .tracer
            .trace
            .provider()
            .block_hash_for_id(block_num.into())
    }

    /// executes the tracing of a given block
    pub fn execute(&self, block_num: u64) -> ParserFuture {
        let parser = self.parser.clone();
        Box::pin(self.executor.spawn_result_task_as(
            async move { parser.execute_block(block_num).await },
            TaskKind::Default
        )) as ParserFuture
    }
}
