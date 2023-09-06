use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use crate::{
    error_trace,
    errors::{TraceParseError, TraceParseErrorKind},
    executor::{Executor, TaskKind},
    init_trace,
    stats::TraceMetricEvent,
    structured_trace::{StructuredTrace, TxTrace},
    success_trace,
};
use alloy_etherscan::Client;
use ethers_core::types::Chain;
use futures::{stream::FuturesUnordered, Stream, StreamExt};
use reth_primitives::{BlockId, BlockNumberOrTag, H256};
use reth_rpc_types::trace::parity::{TraceResultsWithTransactionHash, TraceType, TransactionTrace};
use reth_tracing::TracingClient;
use tokio::{sync::mpsc::UnboundedSender, task::JoinHandle};

use self::parser::{trace_block, TraceParser};

mod parser;
mod utils;

pub(crate) const UNKNOWN: &str = "unknown";
pub(crate) const RECEIVE: &str = "receive";
pub(crate) const FALLBACK: &str = "fallback";
const CACHE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10_000);
const CACHE_DIRECTORY: &str = "./abi_cache";

type BlockNumber = u64;
type TransactionHash = H256;
type TransactionIndex = u16;
type TraceIndex = u16;

pub struct Parser {
    executor: Executor,
    parser_fut: FuturesUnordered<JoinHandle<Option<ParsedType>>>,
    parser: TraceParser,
    metrics_tx: Arc<UnboundedSender<TraceMetricEvent>>,
    tracer: Arc<TracingClient>,
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
        let parser = TraceParser::new(etherscan_client, Arc::clone(&tracer));

        Self {
            executor,
            parser_fut: FuturesUnordered::new(),
            parser,
            metrics_tx: Arc::new(metrics_tx),
            tracer,
        }
    }

    /// executes the tracing of a given block
    pub fn execute(&mut self, block_num: u64) {
        let tracer = self.tracer.clone();
        let metrics_tx = self.metrics_tx.clone();
        self.parser_fut.push(
            self.executor.spawn_result_task_as(
                trace_block(tracer, metrics_tx, block_num),
                TaskKind::Default,
            ),
        );
    }

    /// parses a block and gathers the transactions
    fn parse_block(&mut self, block_trace: Vec<TraceResultsWithTransactionHash>, block_num: u64) {
        for (idx, trace) in block_trace.into_iter().enumerate() {
            let transaction_traces = trace.full_trace.trace;
            let tx_hash = trace.transaction_hash;
            if transaction_traces.is_none() {
                let _ = self.metrics_tx.send(TraceMetricEvent::TxTracingErrorMetric {
                    block_num,
                    tx_hash,
                    tx_idx: idx as u64,
                    error: (&TraceParseError::TracesMissingTx(tx_hash.into())).into(),
                });
                return
            }

            self.parse_transaction(transaction_traces.unwrap(), block_num, tx_hash, idx as u64);
        }
    }

    /// parses a transaction and gathers the traces
    fn parse_transaction(
        &mut self,
        tx_trace: Vec<TransactionTrace>,
        block_num: u64,
        tx_hash: H256,
        tx_idx: u64,
    ) {
        init_trace!(tx_hash, tx_idx, tx_trace.len());
        for (idx, trace) in tx_trace.into_iter().enumerate() {
            self.parse_trace(trace, block_num, tx_hash, tx_idx, idx as u64);
        }
    }

    /// pushes each trace to parser_fut
    fn parse_trace(
        &mut self,
        tx_trace: TransactionTrace,
        block_num: u64,
        tx_hash: H256,
        tx_idx: u64,
        trace_idx: u64,
    ) {
        let parser = self.parser.clone();
        let metrics_tx = self.metrics_tx.clone();
        let fut = async move {
            let structured_trace = parser.parse(tx_trace, tx_hash, block_num).await;
            let err: Option<TraceParseErrorKind> = if let Err(e) = &structured_trace {
                error_trace!(tx_hash, e);
                Some(e.into())
            } else {
                success_trace!(tx_hash);
                None
            };

            let res = if err.is_none() {
                Some(ParsedType::Trace(
                    structured_trace.unwrap(),
                    block_num,
                    tx_hash,
                    tx_idx as u16,
                    trace_idx as u16,
                ))
            } else {
                None
            };

            let _ = metrics_tx.send(TraceMetricEvent::TraceMetricRecieved {
                block_num,
                tx_hash,
                tx_idx,
                tx_trace_idx: trace_idx,
                error: err.map(|e| e.into()),
            });

            res
        };

        self.parser_fut.push(self.executor.spawn_result_task_as(fut, TaskKind::Default));
    }
}

impl Stream for Parser {
    type Item = ThisRet;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        while let Poll::Ready(val) = this.parser_fut.poll_next_unpin(cx) {
            let parsed = match val {
                Some(Err(_)) | None => return Poll::Ready(None),
                Some(Ok(Some(t))) => t,
                _ => continue,
            };

            match parsed {
                ParsedType::Block(trace_results, block_num) => {
                    this.parse_block(trace_results, block_num)
                }
                ParsedType::Trace(trace, block_num, tx_hash, tx_idx, trace_idx) => {
                    return Poll::Ready(Some(ThisRet::new(
                        trace, block_num, tx_hash, tx_idx, trace_idx,
                    )))
                }
                ParsedType::AggregatedTraces(aggr) => (),
            }
        }

        Poll::Pending
    }
}

pub(crate) enum ParsedType {
    Block(Vec<TraceResultsWithTransactionHash>, BlockNumber),
    Trace(StructuredTrace, BlockNumber, TransactionHash, TransactionIndex, TraceIndex),
    AggregatedTraces(TxTrace),
}

pub struct ThisRet {
    trace: StructuredTrace,
    block_num: u64,
    tx_hash: H256,
    tx_idx: u16,
    trace_idx: u16,
}

impl ThisRet {
    fn new(
        trace: StructuredTrace,
        block_num: u64,
        tx_hash: H256,
        tx_idx: u16,
        trace_idx: u16,
    ) -> Self {
        Self { trace, block_num, tx_hash, tx_idx, trace_idx }
    }
}
