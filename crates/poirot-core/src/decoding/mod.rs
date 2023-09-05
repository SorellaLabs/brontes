use std::collections::HashSet;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use crate::executor::TaskKind;
use crate::stats::TraceMetricEvent;
use crate::structured_trace::{StructuredTrace, TxTrace};
use crate::{error_trace, success_trace};
use crate::{errors::TraceParseError, executor::Executor};
use alloy_etherscan::Client;
use futures::stream::FuturesUnordered;
use futures::{Stream, StreamExt};
use reth_primitives::H256;
use reth_primitives::{BlockId, BlockNumberOrTag};
use reth_rpc_types::trace::parity::{TraceResultsWithTransactionHash, TraceType, TransactionTrace};
use reth_tracing::TracingClient;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

use self::parser::TraceParser;

mod parser;
mod utils;

pub(crate) const UNKNOWN: &str = "unknown";
pub(crate) const RECEIVE: &str = "receive";
pub(crate) const FALLBACK: &str = "fallback";
pub(crate) const CACHE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10_000);

type BlockNumber = u64;
type TransactionHash = H256;
type TransactionIndex = u16;
type TraceIndex = u16;

pub struct Parser {
    executor: Executor,
    parser_fut: FuturesUnordered<JoinHandle<Option<ParsedType>>>,
    parser: TraceParser,
    metrics_tx: UnboundedSender<TraceMetricEvent>,
    tracer: Arc<TracingClient>,
}

impl Parser {
    pub fn new(
        metrics_tx: UnboundedSender<TraceMetricEvent>,
        etherscan_client: Client,
        db_path: &Path,
    ) -> Self {
        let executor = Executor::new();
        let tracer = Arc::new(TracingClient::new(db_path, executor.runtime.handle().clone()));
        let parser = TraceParser::new(etherscan_client, Arc::clone(&tracer));
        Self { executor, parser_fut: FuturesUnordered::new(), parser, metrics_tx, tracer }
    }

    /// executes the tracing of a given block
    pub fn execute(&mut self, block_num: u64) {
        // spawns the task to get the txs and traces
        self.parser_fut.push(
            self.executor.spawn_result_task_as(self.trace_block(block_num), TaskKind::Default),
        );
    }

    /// traces a block into a vec of tx traces
    async fn trace_block(&self, block_num: u64) -> Option<ParsedType> {
        let mut trace_type = HashSet::new();
        trace_type.insert(TraceType::Trace);

        let parity_trace = self
            .tracer
            .trace
            .replay_block_transactions(
                BlockId::Number(BlockNumberOrTag::Number(block_num)),
                trace_type,
            )
            .await
            .map_err(|e| Into::<TraceParseError>::into(e));

        match parity_trace {
            Ok(Some(trace)) => return Some(ParsedType::Block(trace, block_num)),
            Ok(None) => self.metrics_tx.send(TraceMetricEvent::BlockTracingErrorMetric {
                block_num,
                error: TraceParseError::TracesMissingBlock(block_num).into(),
            }),
            Err(e) => self
                .metrics_tx
                .send(TraceMetricEvent::BlockTracingErrorMetric { block_num, error: e.into() }),
        };

        None
    }

    /// parses a block and gathers the transactions
    fn parse_block(&mut self, block_trace: Vec<TraceResultsWithTransactionHash>, block_num: u64) {
        for (idx, trace) in block_trace.into_iter().enumerate() {
            let transaction_traces = trace.full_trace.trace;
            let tx_hash = trace.transaction_hash;
            if transaction_traces.is_none() {
                self.metrics_tx.send(TraceMetricEvent::TxTracingErrorMetric {
                    block_num,
                    tx_hash,
                    tx_idx: idx as u64,
                    error: TraceParseError::TracesMissingTx(tx_hash.into()).into(),
                });
                return;
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
        let fut = async {
            let structured_trace = self.parser.parse(tx_trace, tx_hash, block_num).await;
            let err: Option<TraceParseError> = if let Err(e) = structured_trace {
                error_trace!(tx_hash, e);
                Some(e)
            } else {
                success_trace!(tx_hash);
                None
            };

            self.metrics_tx.send(TraceMetricEvent::TraceMetricRecieved {
                block_num,
                tx_hash,
                tx_idx,
                tx_trace_idx: trace_idx,
                error: err.map(|e| e.into()),
            });

            if err.is_none() {
                return Some(ParsedType::Trace(
                    structured_trace.unwrap(),
                    block_num,
                    tx_hash,
                    tx_idx as u16,
                    trace_idx as u16,
                ));
            }

            None
        };

        self.parser_fut.push(self.executor.spawn_result_task_as(fut, TaskKind::Default));
    }
}

impl Stream for Parser {
    type Item = TxTrace;

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
                ParsedType::Trace(trace, block_num, tx_hash, tx_idx, trace_idx) => todo!(),
                ParsedType::AggregatedTraces(aggr) => return Poll::Ready(Some(aggr)),
            }
        }

        Poll::Pending
    }
}

enum ParsedType {
    Block(Vec<TraceResultsWithTransactionHash>, BlockNumber),
    Trace(StructuredTrace, BlockNumber, TransactionHash, TransactionIndex, TraceIndex),
    AggregatedTraces(TxTrace),
}
