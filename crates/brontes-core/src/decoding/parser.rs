use std::time::Duration;

#[cfg(feature = "dyn-decode")]
use alloy_json_abi::JsonAbi;
#[cfg(feature = "dyn-decode")]
use alloy_primitives::Address;
use brontes_metrics::trace::types::{BlockStats, TraceParseErrorKind, TransactionStats};
#[cfg(feature = "dyn-decode")]
use brontes_types::FastHashMap;
use futures::future::join_all;
use reth_primitives::BlockHash;
#[cfg(feature = "dyn-decode")]
use reth_rpc_types::trace::parity::Action;
use reth_rpc_types::{AnyReceiptEnvelope, Log, TransactionReceipt};
use tracing::error;
#[cfg(feature = "dyn-decode")]
use tracing::info;

use super::*;
#[cfg(feature = "dyn-decode")]
use crate::decoding::dyn_decode::decode_input_with_abi;
use crate::errors::TraceParseError;

/// A [`TraceParser`] will iterate through a block's Parity traces and attempt
/// to decode each call for later analysis.
pub struct TraceParser<T: TracingProvider, DB: LibmdbxReader + DBWriter> {
    libmdbx:               &'static DB,
    pub tracer:            Arc<T>,
    pub(crate) metrics_tx: Arc<UnboundedSender<ParserMetricEvents>>,
}

impl<T: TracingProvider, DB: LibmdbxReader + DBWriter> Clone for TraceParser<T, DB> {
    fn clone(&self) -> Self {
        Self {
            libmdbx:    self.libmdbx,
            tracer:     self.tracer.clone(),
            metrics_tx: self.metrics_tx.clone(),
        }
    }
}

impl<T: TracingProvider, DB: LibmdbxReader + DBWriter> TraceParser<T, DB> {
    pub async fn new(
        libmdbx: &'static DB,
        tracer: Arc<T>,
        metrics_tx: Arc<UnboundedSender<ParserMetricEvents>>,
    ) -> Self {
        Self { libmdbx, tracer, metrics_tx }
    }

    pub fn get_tracer(&self) -> Arc<T> {
        self.tracer.clone()
    }

    pub async fn load_block_from_db(&self, block_num: u64) -> Option<(Vec<TxTrace>, Header)> {
        let mut traces = self.libmdbx.load_trace(block_num).ok()?;
        traces.sort_by(|a, b| a.tx_index.cmp(&b.tx_index));
        traces.dedup_by(|a, b| a.tx_index.eq(&b.tx_index));

        Some((traces, self.tracer.header_by_number(block_num).await.ok()??))
    }

    pub async fn trace_clickhouse_block(self, block_num: u64) {
        let parity_trace = self.trace_block(block_num).await;
        let receipts = self.get_receipts(block_num).await;

        if parity_trace.0.is_none() && receipts.0.is_none() {
            return
        }

        #[cfg(feature = "dyn-decode")]
        let traces = self
            .fill_metadata(parity_trace.0.unwrap(), parity_trace.1, receipts.0.unwrap(), block_num)
            .await;
        #[cfg(not(feature = "dyn-decode"))]
        let traces = self
            .fill_metadata(parity_trace.0.unwrap(), receipts.0.unwrap(), block_num)
            .await;

        let mut cnt = 0;

        while self
            .libmdbx
            .save_traces(block_num, traces.0.clone())
            .await
            .is_err()
        {
            cnt += 1;
            if cnt > 20 {
                error!(%block_num, "attempted 20 inserts for db but all failed");
                break
            }

            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    }

    /// executes the tracing of a given block
    #[allow(unreachable_code)]
    pub async fn execute_block(self, block_num: u64) -> Option<(BlockHash, Vec<TxTrace>, Header)> {
        if let Some(res) = self.load_block_from_db(block_num).await {
            tracing::debug!(%block_num, traces_in_block= res.0.len(),"loaded trace for db");

            let block_hash = self.tracer.block_hash_for_id(block_num).await.ok()?;

            if block_hash.is_none() {
                error!(%block_num, "failed to get block hash for block");
            }

            return block_hash.map(|b| (b, res.0, res.1))
        }

        tracing::info!(target: "brontes", "no block found in db, tracing block: {:?}", block_num);
        let parity_trace = self.trace_block(block_num).await;
        let receipts = self.get_receipts(block_num).await;

        if parity_trace.0.is_none() && receipts.0.is_none() {
            #[cfg(feature = "dyn-decode")]
            self.metrics_tx
                .send(TraceMetricEvent::BlockMetricRecieved(parity_trace.2).into())
                .unwrap();
            #[cfg(not(feature = "dyn-decode"))]
            let _ = self
                .metrics_tx
                .send(TraceMetricEvent::BlockMetricRecieved(parity_trace.1).into());
            return None
        }
        #[cfg(feature = "dyn-decode")]
        let traces = self
            .fill_metadata(parity_trace.0?, parity_trace.1, receipts.0?, block_num)
            .await;
        #[cfg(not(feature = "dyn-decode"))]
        let traces = self
            .fill_metadata(parity_trace.0?, receipts.0?, block_num)
            .await;

        let _ = self
            .metrics_tx
            .send(TraceMetricEvent::BlockMetricRecieved(traces.1).into());

        if self
            .libmdbx
            .save_traces(block_num, traces.0.clone())
            .await
            .is_err()
        {
            error!(%block_num, "failed to store traces for block");
        }

        let block_hash = self.tracer.block_hash_for_id(block_num).await.ok()?;

        if block_hash.is_none() {
            error!(%block_num, "failed to get block hash for block");
        }

        block_hash.map(|b| (b, traces.0, traces.2))
    }

    #[allow(unreachable_code)]
    pub async fn execute_block_discovery(
        self,
        block_num: u64,
    ) -> Option<(BlockHash, Vec<TxTrace>, Header)> {
        if let Some(res) = self.load_block_from_db(block_num).await {
            tracing::debug!(%block_num, traces_in_block= res.0.len(),"loaded trace for db");

            let block_hash = self.tracer.block_hash_for_id(block_num).await.ok()?;

            if block_hash.is_none() {
                error!(%block_num, "failed to get block hash for block");
            }

            return block_hash.map(|b| (b, res.0, res.1))
        }

        let parity_trace = self.trace_block(block_num).await;
        let receipts = self.get_receipts(block_num).await;

        if parity_trace.0.is_none() && receipts.0.is_none() {
            #[cfg(feature = "dyn-decode")]
            self.metrics_tx
                .send(TraceMetricEvent::BlockMetricRecieved(parity_trace.2).into())
                .unwrap();
            #[cfg(not(feature = "dyn-decode"))]
            let _ = self
                .metrics_tx
                .send(TraceMetricEvent::BlockMetricRecieved(parity_trace.1).into());
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

        let block_hash = self.tracer.block_hash_for_id(block_num).await.ok()?;

        if block_hash.is_none() {
            error!(%block_num, "failed to get block hash for block");
        }

        block_hash.map(|b| (b, traces.0, traces.2))
    }

    #[cfg(feature = "dyn-decode")]
    /// traces a block into a vec of tx traces
    pub(crate) async fn trace_block(
        &self,
        block_num: u64,
    ) -> (Option<Vec<TxTrace>>, FastHashMap<Address, JsonAbi>, BlockStats) {
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
                    t.trace
                        .iter()
                        .filter_map(|inner| match &inner.trace.action {
                            Action::Call(call) => Some(call.to),
                            _ => None,
                        })
                })
                .filter(|addr| self.libmdbx.get_protocol(*addr).is_err())
                .collect::<Vec<Address>>();
            info!("addresses for dyn decoding: {:#?}", addresses);
            //self.libmdbx.get_abis(addresses).await.unwrap()
            FastHashMap::default()
        } else {
            FastHashMap::default()
        };

        info!("{:#?}", json);

        (trace, json, stats)
    }

    #[cfg(not(feature = "dyn-decode"))]
    pub(crate) async fn trace_block(&self, block_num: u64) -> (Option<Vec<TxTrace>>, BlockStats) {
        tracing::info!(target: "brontes", "tracing block: {:?}", block_num);
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
    ) -> (Option<Vec<TransactionReceipt<AnyReceiptEnvelope<Log>>>>, BlockStats) {
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
        #[cfg(feature = "dyn-decode")] dyn_json: FastHashMap<Address, JsonAbi>,
        block_receipts: Vec<TransactionReceipt<AnyReceiptEnvelope<Log>>>,
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
                        receipt.transaction_index.unwrap(),
                        receipt.gas_used,
                        receipt.effective_gas_price,
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
        #[cfg(feature = "dyn-decode")] dyn_json: &FastHashMap<Address, JsonAbi>,
        block_num: u64,
        tx_hash: B256,
        tx_idx: u64,
        gas_used: u128,
        effective_gas_price: u128,
    ) -> (TxTrace, TransactionStats) {
        let stats = TransactionStats {
            block_num,
            tx_hash,
            tx_idx: tx_idx as u16,
            traces: vec![],
            err: None,
        };

        #[cfg(feature = "dyn-decode")]
        tx_trace.trace.iter_mut().for_each(|iter| {
            let addr = match iter.trace.action {
                Action::Call(ref addr) => addr.to,
                _ => return,
            };

            if let Some(json_abi) = dyn_json.get(&addr) {
                let decoded_calldata = decode_input_with_abi(json_abi, &iter.trace).ok().flatten();
                iter.decoded_data = decoded_calldata;
            }
        });

        tx_trace.effective_price = effective_gas_price;
        tx_trace.gas_used = gas_used;

        (tx_trace, stats)
    }
}
