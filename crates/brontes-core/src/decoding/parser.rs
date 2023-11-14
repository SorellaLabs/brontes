use std::{collections::HashMap, sync::Arc};

use alloy_etherscan::Client;
use alloy_json_abi::JsonAbi;
use brontes_database::database::Database;
use brontes_metrics::{
    trace::types::{BlockStats, TraceParseErrorKind, TraceStats, TransactionStats},
    PoirotMetricEvents,
};
use futures::future::join_all;
use reth_primitives::{Header, H160, H256};
use reth_rpc_types::{
    trace::parity::{
        Action as RethAction, CallAction as RethCallAction, TraceResultsWithTransactionHash,
        TraceType, TransactionTrace, VmTrace,
    },
    Log, TransactionReceipt,
};

use super::*;
use crate::{
    decoding::{dyn_decode::decode_input_with_abi, vm_linker::link_vm_to_trace},
    errors::TraceParseError,
};

/// A [`TraceParser`] will iterate through a block's Parity traces and attempt
/// to decode each call for later analysis.
#[derive(Clone)]
pub struct TraceParser<'db, T: TracingProvider> {
    database:              &'db Database,
    should_fetch:          Box<dyn Fn(&H160) -> bool>,
    pub tracer:            Arc<T>,
    pub(crate) metrics_tx: Arc<UnboundedSender<PoirotMetricEvents>>,
}

impl<'db, T: TracingProvider> TraceParser<'db, T> {
    pub fn new(
        database: &'db Database,
        should_fetch: Box<dyn Fn(Address) -> bool>,
        tracer: Arc<T>,
        metrics_tx: Arc<UnboundedSender<PoirotMetricEvents>>,
    ) -> Self {
        Self { database, tracer, metrics_tx, should_fetch }
    }

    /// executes the tracing of a given block
    pub async fn execute_block(&self, block_num: u64) -> Option<(Vec<TxTrace>, Header)> {
        let parity_trace = self.trace_block(block_num).await;
        let receipts = self.get_receipts(block_num).await;

        if parity_trace.0.is_none() && receipts.0.is_none() {
            self.metrics_tx
                .send(TraceMetricEvent::BlockMetricRecieved(parity_trace.2).into())
                .unwrap();
            return None
        }
        let traces = self
            .parse_block(parity_trace.0.unwrap(), parity_trace.1, receipts.0.unwrap(), block_num)
            .await;
        self.metrics_tx
            .send(TraceMetricEvent::BlockMetricRecieved(traces.1).into())
            .unwrap();
        Some((traces.0, traces.2))
    }

    /// traces a block into a vec of tx traces
    pub(crate) async fn trace_block(
        &self,
        block_num: u64,
    ) -> (Option<Vec<TraceResultsWithTransactionHash>>, HashMap<H160, JsonAbi>, BlockStats) {
        let mut trace_type = HashSet::new();
        trace_type.insert(TraceType::Trace);
        trace_type.insert(TraceType::VmTrace);

        let parity_trace = self
            .tracer
            .replay_block_transactions(
                BlockId::Number(BlockNumberOrTag::Number(block_num)),
                trace_type,
            )
            .await;

        let mut stats = BlockStats::new(block_num, None);
        let trace = match parity_trace {
            Ok(Some(t)) => Some(t),
            Ok(None) => {
                stats.err = Some(TraceParseErrorKind::TracesMissingBlock);
                None
            }
            Err(e) => {
                stats.err = Some((&Into::<TraceParseError>::into(e)).into());
                None
            }
        };

        let json = if let Some(trace) = &trace {
            let addresses = trace
                .iter()
                .flat_map(|t| {
                    t.full_trace
                        .trace
                        .iter()
                        .filter_map(|inner| match &inner.action {
                            RethAction::Call(call) => Some(call.to),
                            _ => None,
                        })
                })
                .filter(|addr| (self.should_fetch)(addr))
                .collect::<Vec<H160>>();
            self.database.get_abis(addresses).await
        } else {
            HashMap::default()
        };

        (trace, json, stats)
    }

    /// gets the transaction $receipts for a block
    pub(crate) async fn get_receipts(
        &self,
        block_num: u64,
    ) -> (Option<Vec<TransactionReceipt>>, BlockStats) {
        let tx_receipts = self
            .tracer
            .block_receipts(BlockNumberOrTag::Number(block_num))
            .await;
        let mut stats = BlockStats::new(block_num, None);

        let receipts = match tx_receipts {
            Ok(Some(t)) => Some(t),
            Ok(None) => {
                stats.err = Some(TraceParseErrorKind::TracesMissingBlock);
                None
            }
            _ => None,
        };

        (receipts, stats)
    }

    pub(crate) async fn parse_block(
        &self,
        block_trace: Vec<TraceResultsWithTransactionHash>,
        dyn_json: HashMap<H160, JsonAbi>,
        block_receipts: Vec<TransactionReceipt>,
        block_num: u64,
    ) -> (Vec<TxTrace>, BlockStats, Header) {
        let mut stats = BlockStats::new(block_num, None);

        let (traces, tx_stats): (Vec<_>, Vec<_>) =
            join_all(block_trace.into_iter().zip(block_receipts.into_iter()).map(
                |(trace, receipt)| {
                    let transaction_traces = trace.full_trace.trace;
                    let vm_traces = trace.full_trace.vm_trace.unwrap();

                    let tx_hash = trace.transaction_hash;

                    self.parse_transaction(
                        transaction_traces,
                        &dyn_json,
                        vm_traces,
                        receipt.logs,
                        block_num,
                        tx_hash,
                        receipt.transaction_index.try_into().unwrap(),
                        receipt.gas_used.unwrap().to(),
                        receipt.effective_gas_price.to(),
                    )
                },
            ))
            .await
            .into_iter()
            .unzip();

        stats.txs = tx_stats;
        stats.trace();

        (
            traces,
            stats,
            self.tracer
                .header_by_number(block_num)
                .await
                .unwrap()
                .unwrap(),
        )
    }

    /// parses a transaction and gathers the traces
    async fn parse_transaction(
        &self,
        tx_trace: Vec<TransactionTrace>,
        dyn_json: &HashMap<H160, JsonAbi>,
        vm: VmTrace,
        logs: Vec<Log>,
        block_num: u64,
        tx_hash: H256,
        tx_idx: u64,
        gas_used: u64,
        effective_gas_price: u64,
    ) -> (TxTrace, TransactionStats) {
        init_trace!(tx_hash, tx_idx, tx_trace.len());
        let mut traces = Vec::new();
        let mut stats = TransactionStats {
            block_num,
            tx_hash,
            tx_idx: tx_idx as u16,
            traces: vec![],
            err: None,
        };

        let len = tx_trace.len();

        let mut linked_trace = link_vm_to_trace(vm, tx_trace, logs);

        let mut linked_trace: Vec<TransactionTraceWithLogs> = linked_trace
            .into_iter()
            .map(|iter| {
                let addr = match iter.trace.action {
                    RethAction::Call(addr) => addr.to,
                    _ => return iter,
                };

                if let Some(json_abi) = dyn_json.get(&addr) {
                    let decoded_calldata =
                        decode_input_with_abi(json_abi, &iter.trace).ok().flatten();
                    iter.decoded_data = decoded_calldata;
                }

                iter
            })
            .collect();

        for (idx, trace) in linked_trace.into_iter().enumerate() {
            let mut stat = TraceStats::new(block_num, tx_hash, tx_idx as u16, idx as u16, None);
            if let Err(e) = abi_trace {
                stat.err = Some(Into::<TraceParseErrorKind>::into(&e));
            }
            traces.push(trace);
            stat.trace(len);
            stats.traces.push(stat);
        }

        stats.trace();
        (TxTrace::new(traces, tx_hash, tx_idx, gas_used, effective_gas_price), stats)
    }
}

#[cfg(test)]
mod tests {

    use dotenv::dotenv;
    use serial_test::serial;
    use tokio::sync::mpsc::unbounded_channel;

    use super::*;
    use crate::test_utils::*;

    #[tokio::test]
    #[serial]
    async fn test_execute_block() {
        dotenv().ok();

        let (tx, _rx) = unbounded_channel();

        let tracer = init_trace_parser(tokio::runtime::Handle::current().clone(), tx);

        let block_1 = tracer.execute_block(17000000).await;
        assert!(block_1.is_some());

        let mut traces = block_1.unwrap().0;
        assert_eq!(traces.len(), 102);

        let mut txs: Vec<TestTxTrace> = join_all(
            traces
                .iter()
                .map(|t| async {
                    let full_trace = get_full_tx_trace(t.tx_hash.clone()).await;
                    let receipt = get_tx_reciept(t.tx_hash.clone()).await;

                    let traces_with_logs = link_vm_to_trace(
                        full_trace.vm_trace.unwrap(),
                        full_trace.trace,
                        receipt.logs,
                    );

                    TxTrace::new(
                        traces_with_logs,
                        receipt.transaction_hash.unwrap(),
                        receipt.transaction_index.as_u64(),
                        receipt.gas_used.unwrap().to::<u64>(),
                        receipt.effective_gas_price.to::<u64>(),
                    )
                    .into()
                })
                .collect::<Vec<_>>(),
        )
        .await;

        txs.sort_by(|a, b| a.tx_hash.cmp(&b.tx_hash));
        traces.sort_by(|a, b| a.tx_hash.cmp(&b.tx_hash));
        assert_eq!(traces.len(), txs.len());

        for (trace, test_trace) in txs.into_iter().zip(traces) {
            assert_eq!(trace.tx_hash, test_trace.tx_hash);
            for inner_trace in test_trace.trace {
                assert!(trace.trace.contains(&inner_trace.into()));
            }
        }
    }
}
