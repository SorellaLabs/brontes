use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use crate::{
    errors::TraceParseErrorKind,
    executor::{Executor, TaskKind},
    init_trace,
    stats::TraceMetricEvent,
};
use poirot_types::structured_trace::TxTrace;

use alloy_etherscan::Client;
use ethers_core::types::Chain;
use futures::{stream::FuturesUnordered, Stream, StreamExt};
use reth_primitives::{BlockId, BlockNumberOrTag, H256};
use reth_rpc_types::trace::parity::{TraceResultsWithTransactionHash, TraceType, TransactionTrace};
use reth_tracing::TracingClient;
use tokio::{sync::mpsc::UnboundedSender, task::JoinHandle};

use self::parser::TraceParser;

mod parser;

pub(crate) const UNKNOWN: &str = "unknown";
pub(crate) const RECEIVE: &str = "receive";
pub(crate) const FALLBACK: &str = "fallback";
const CACHE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10_000);
const CACHE_DIRECTORY: &str = "./abi_cache";

pub struct Parser {
    executor: Executor,
    parser_fut: FuturesUnordered<JoinHandle<Option<Vec<TxTrace>>>>,
    parser: Arc<TraceParser>,
}

impl Parser {
    pub fn new(
        metrics_tx: UnboundedSender<TraceMetricEvent>,
        etherscan_key: &str,
        db_path: &str,
    ) -> Self {
        let executor = Executor::new();
        let tracer =
            Arc::new(TracingClient::new(&Path::new(db_path), executor.runtime.handle().clone()));

        let etherscan_client = Client::new_cached(
            Chain::Mainnet,
            etherscan_key,
            Some(PathBuf::from(CACHE_DIRECTORY)),
            CACHE_TIMEOUT,
        )
        .unwrap();
        let parser = TraceParser::new(etherscan_client, Arc::clone(&tracer), Arc::new(metrics_tx));

        Self { executor, parser_fut: FuturesUnordered::new(), parser: Arc::new(parser) }
    }

    /// executes the tracing of a given block OR tx hash OR trace idx in a tx
    pub fn execute(&self, identifier: TypeToParse) {
        match identifier {
            TypeToParse::Block(block_num) => {
                self.parser_fut.push(self.executor.spawn_result_task_as(
                    Self::execute_block(self.parser.clone(), block_num),
                    TaskKind::Default,
                ))
            }
            TypeToParse::Tx(_) => panic!("NOT IMPLEMENTED YET"),
            TypeToParse::TxTrace(_) => panic!("NOT IMPLEMENTED YET"),
        }
    }

    /// executes the tracing of a given block
    async fn execute_block(this: self::Arc<TraceParser>, block_num: u64) -> Option<Vec<TxTrace>> {
        let parity_trace = this.trace_block(block_num).await;

        if parity_trace.0.is_none() {
            this.metrics_tx.send(TraceMetricEvent::BlockMetricRecieved(parity_trace.1)).unwrap();
            return None
        }

        let traces = this.parse_block(parity_trace.0.unwrap(), block_num).await;
        this.metrics_tx.send(TraceMetricEvent::BlockMetricRecieved(traces.1)).unwrap();
        Some(traces.0)
    }
}

impl Stream for Parser {
    type Item = Vec<TxTrace>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        while let Poll::Ready(val) = this.parser_fut.poll_next_unpin(cx) {
            match val {
                Some(Err(_)) | None => return Poll::Ready(None),
                Some(Ok(None)) => panic!("NOT IMPLEMENTED YET"),
                Some(Ok(Some(parsed_type))) => return Poll::Ready(Some(parsed_type)),
            };
        }

        Poll::Pending
    }
}

/// types of traces that can be executed
pub enum TypeToParse {
    Block(u64),
    Tx(H256),             // if we want to parser a single tx
    TxTrace((H256, u16)), // if we want to parser a single trace in a tx
}
