#[cfg(feature = "dyn-decode")]
use std::collections::HashMap;
use std::sync::Arc;

#[cfg(feature = "dyn-decode")]
use alloy_json_abi::JsonAbi;
use brontes_database::database::Database;
use brontes_metrics::{
    trace::types::{BlockStats, TraceParseErrorKind, TraceStats, TransactionStats},
    PoirotMetricEvents,
};
use futures::future::join_all;
use reth_primitives::{Address, Header, B256};
#[cfg(feature = "dyn-decode")]
use reth_rpc_types::trace::parity::{Action, TraceResultsWithTransactionHash};
use reth_rpc_types::TransactionReceipt;

use super::*;
#[cfg(feature = "dyn-decode")]
use crate::{decoding::dyn_decode::decode_input_with_abi, errors::TraceParseError};

/// A [`TraceParser`] will iterate through a block's Parity traces and attempt
/// to decode each call for later analysis.
//#[derive(Clone)]
pub struct TraceParser<'db, T: TracingProvider> {
    #[allow(unused)]
    database:              &'db Database,
    #[allow(unused)]
    should_fetch:          Box<dyn Fn(&Address) -> bool + Send + Sync>,
    pub tracer:            Arc<T>,
    pub(crate) metrics_tx: Arc<UnboundedSender<PoirotMetricEvents>>,
}

impl<'db, T: TracingProvider> TraceParser<'db, T> {
    pub fn new(
        database: &'db Database,
        should_fetch: Box<dyn Fn(&Address) -> bool + Send + Sync>,
        tracer: Arc<T>,
        metrics_tx: Arc<UnboundedSender<PoirotMetricEvents>>,
    ) -> Self {
        Self { database, tracer, metrics_tx, should_fetch }
    }

    /// executes the tracing of a given block
    pub async fn execute_block(&'db self, block_num: u64) -> Option<(Vec<TxTrace>, Header)> {
        let parity_trace = self.trace_block(block_num).await;
        let receipts = self.get_receipts(block_num).await;

        if parity_trace.0.is_none() && receipts.0.is_none() {
            #[cfg(feature = "dyn-decode")]
            self.metrics_tx
                .send(TraceMetricEvent::BlockMetricRecieved(parity_trace.2).into())
                .unwrap();
            #[cfg(not(feature = "dyn-decode"))]
            self.metrics_tx
                .send(TraceMetricEvent::BlockMetricRecieved(parity_trace.1).into())
                .unwrap();
            return None
        }
        #[cfg(feature = "dyn-decode")]
        let traces = self
            .fill_metadata(parity_trace.0.unwrap(), parity_trace.1, receipts.0.unwrap(), block_num)
            .await;
        #[cfg(not(feature = "dyn-decode"))]
        let traces = self
            .fill_metadata(parity_trace.0.unwrap(), receipts.0.unwrap(), block_num)
            .await;

        self.metrics_tx
            .send(TraceMetricEvent::BlockMetricRecieved(traces.1).into())
            .unwrap();
        Some((traces.0, traces.2))
    }

    #[cfg(feature = "dyn-decode")]
    /// traces a block into a vec of tx traces
    pub(crate) async fn trace_block(
        &self,
        block_num: u64,
    ) -> (Option<Vec<TraceResultsWithTransactionHash>>, HashMap<Address, JsonAbi>, BlockStats) {
        let merged_trace = self
            .tracer
            .replay_block_transactions(BlockId::Number(BlockNumberOrTag::Number(block_num)))
            .await;

        let mut stats = BlockStats::new(block_num, None);
        let trace = match merged_trace {
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
                            Action::Call(call) => Some(call.to),
                            _ => None,
                        })
                })
                .filter(|addr| (self.should_fetch)(addr))
                .collect::<Vec<Address>>();
            self.database.get_abis(addresses).await
        } else {
            HashMap::default()
        };

        (trace, json, stats)
    }

    #[cfg(not(feature = "dyn-decode"))]
    pub(crate) async fn trace_block(&self, block_num: u64) -> (Option<Vec<TxTrace>>, BlockStats) {
        let merged_trace = self
            .tracer
            .replay_block_transactions(BlockId::Number(BlockNumberOrTag::Number(block_num)))
            .await;

        let mut stats = BlockStats::new(block_num, None);
        let trace = match merged_trace {
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

        (trace, stats)
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

    pub(crate) async fn fill_metadata(
        &self,
        block_trace: Vec<TxTrace>,
        #[cfg(feature = "dyn-decode")] dyn_json: HashMap<Address, JsonAbi>,
        block_receipts: Vec<TransactionReceipt>,
        block_num: u64,
    ) -> (Vec<TxTrace>, BlockStats, Header) {
        let mut stats = BlockStats::new(block_num, None);

        let (traces, tx_stats): (Vec<_>, Vec<_>) =
            join_all(block_trace.into_iter().zip(block_receipts.into_iter()).map(
                |(trace, receipt)| {
                    let tx_hash = trace.tx_hash;

                    self.parse_transaction(
                        trace,
                        #[cfg(feature = "dyn-decode")]
                        &dyn_json,
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
        mut tx_trace: TxTrace,
        #[cfg(feature = "dyn-decode")] dyn_json: &HashMap<Address, JsonAbi>,
        block_num: u64,
        tx_hash: B256,
        tx_idx: u64,
        gas_used: u64,
        effective_gas_price: u64,
    ) -> (TxTrace, TransactionStats) {
        init_trace!(tx_hash, tx_idx, tx_trace.trace.len());
        let mut stats = TransactionStats {
            block_num,
            tx_hash,
            tx_idx: tx_idx as u16,
            traces: vec![],
            err: None,
        };

        #[cfg(feature = "dyn-decode")]
        tx_trace.trace.iter_mut().for_each(|mut iter| {
            let addr = match iter.trace.action {
                Action::Call(ref addr) => addr.to,
                _ => return,
            };

            if let Some(json_abi) = dyn_json.get(&addr) {
                let decoded_calldata = decode_input_with_abi(json_abi, &iter.trace).ok().flatten();
                iter.decoded_data = decoded_calldata;
            }
        });

        let len = tx_trace.trace.len();

        for idx in 0..tx_trace.trace.len() {
            let stat = TraceStats::new(block_num, tx_hash, tx_idx as u16, idx as u16, None);
            stat.trace(len);
            stats.traces.push(stat);
        }

        stats.trace();
        tx_trace.effective_price = effective_gas_price;
        tx_trace.gas_used = gas_used;

        (tx_trace, stats)
    }
}

#[cfg(test)]
mod tests {
    use std::{str::FromStr, time::SystemTime};

    use brontes_classifier::Classifier;
    use brontes_database::database::Database;
    use brontes_types::test_utils::write_tree_as_json;
    use serial_test::serial;
    use tokio::sync::mpsc::unbounded_channel;

    use super::*;
    use crate::{init_tracing, test_utils::init_trace_parser};

    // #[cfg(feature = "dyn-decode")]
    #[tokio::test]
    #[serial]
    async fn test_dyn_decode() {
        dotenv::dotenv().ok();
        init_tracing();
        let block_num = 18522278;

        let (tx, _rx) = unbounded_channel();

        let tracer = init_trace_parser(tokio::runtime::Handle::current().clone(), tx);
        let (trace, stats) = tracer.trace_block(block_num).await;
        info!("{:#?}", trace.unwrap());
    }
}
