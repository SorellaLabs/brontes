use crate::{
    decoding::utils::*,
    errors::TraceParseError,
    stats::types::{BlockStats, TraceStats, TransactionStats},
};
use alloy_etherscan::Client;
use alloy_json_abi::JsonAbi;
use reth_provider::ReceiptProvider;
use reth_tracing::TracingClient;

use super::{utils::IDiamondLoupe::facetAddressCall, *};
use reth_primitives::{Log, H256};
use reth_rpc_types::{
    trace::parity::{Action as RethAction, CallAction as RethCallAction},
    CallRequest,
};
use std::sync::Arc;

use alloy_sol_types::SolCall;
use reth_primitives::Bytes;

use reth_rpc::eth::revm_utils::EvmOverrides;

#[derive(Clone)]
/// A [`TraceParser`] will iterate through a block's Parity traces and attempt to decode each call
/// for later analysis.
pub(crate) struct TraceParser {
    etherscan_client: Client,
    tracer: Arc<TracingClient>,
    pub(crate) metrics_tx: Arc<UnboundedSender<TraceMetricEvent>>,
}

impl TraceParser {
    pub fn new(
        etherscan_client: Client,
        tracer: Arc<TracingClient>,
        metrics_tx: Arc<UnboundedSender<TraceMetricEvent>>,
    ) -> Self {
        Self { etherscan_client, tracer, metrics_tx }
    }

    /// traces a block into a vec of tx traces
    pub(crate) async fn trace_block(
        &self,
        block_num: u64,
    ) -> (Option<Vec<TraceResultsWithTransactionHash>>, BlockStats) {
        let mut trace_type = HashSet::new();
        trace_type.insert(TraceType::Trace);

        let parity_trace = self
            .tracer
            .trace
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

        (trace, stats)
    }

    /// parses a block and gathers the transactions
    pub(crate) async fn parse_block(
        &self,
        block_trace: Vec<TraceResultsWithTransactionHash>,
        block_num: u64,
    ) -> (Vec<TxTrace>, BlockStats) {
        let mut traces = Vec::new();
        let mut stats = BlockStats::new(block_num, None);
        for (idx, trace) in block_trace.into_iter().enumerate() {
            let transaction_traces = trace.full_trace.trace;
            let tx_hash = trace.transaction_hash;
            let logs = self.tracer.api.provider().receipt_by_hash(tx_hash).unwrap().unwrap().logs;
            if transaction_traces.is_none() {
                traces.push(TxTrace::new(vec![], tx_hash, logs.clone(), idx as usize));
                stats.txs.push(TransactionStats {
                    block_num,
                    tx_hash,
                    tx_idx: idx as u16,
                    traces: vec![],
                    err: Some(TraceParseErrorKind::TracesMissingTx),
                });
                continue
            }

            let tx_traces = self
                .parse_transaction(
                    transaction_traces.unwrap(),
                    logs.clone(),
                    block_num,
                    tx_hash,
                    idx as u16,
                )
                .await;
            traces.push(tx_traces.0);
            stats.txs.push(tx_traces.1);
        }

        stats.trace();
        (traces, stats)
    }

    /// parses a transaction and gathers the traces
    async fn parse_transaction(
        &self,
        tx_trace: Vec<TransactionTrace>,
        logs: Vec<Log>,
        block_num: u64,
        tx_hash: H256,
        tx_idx: u16,
    ) -> (TxTrace, TransactionStats) {
        init_trace!(tx_hash, tx_idx, tx_trace.len());
        let mut traces = Vec::new();
        let mut stats = TransactionStats { block_num, tx_hash, tx_idx, traces: vec![], err: None };

        let len = tx_trace.len();
        for (idx, trace) in tx_trace.into_iter().enumerate() {
            let abi_trace = self.update_abi_cache(trace.clone(), block_num, tx_hash).await;
            let mut stat = TraceStats::new(block_num, tx_hash, tx_idx, idx as u16, None);
            if let Err(e) = abi_trace {
                stat.err = Some(Into::<TraceParseErrorKind>::into(&e));
            } else {
                traces.push(trace);
            }
            stat.trace(len);
            stats.traces.push(stat);
        }

        stats.trace();
        (TxTrace::new(traces, tx_hash, logs, tx_idx as usize), stats)
    }

    /// pushes each trace to parser_fut
    async fn update_abi_cache(
        &self,
        trace: TransactionTrace,
        block_num: u64,
        tx_hash: H256,
    ) -> Result<(), TraceParseError> {
        let (action, trace_address) = if let RethAction::Call(call) = trace.action {
            (call, trace.trace_address)
        } else {
            return Ok(())
        };

        //let binding = StaticBindings::Curve_Crypto_Factory_V2;
        let addr = format!("{:#x}", action.from);
        let abi = //if let Some(abi_path) = PROTOCOL_ADDRESS_MAPPING.get(&addr) {
            //serde_json::from_str(abi_path).map_err(|e| TraceParseError::AbiParseError(e))?
        //} else {
            self.etherscan_client.contract_abi(action.to.into()).await?;
        //};

        // Check if the input is empty, indicating a potential `receive` or `fallback` function
        // call.
        if action.input.is_empty() {
            return Ok(())
        }

        let _ =
            self.abi_decoding_pipeline(&abi, &action, &trace_address, &tx_hash, block_num).await;
        return Ok(())
    }

    /// cycles through all possible abi decodings
    /// 1) regular
    /// 2) proxy
    /// 3) diamond proxy
    async fn abi_decoding_pipeline(
        &self,
        abi: &JsonAbi,
        action: &RethCallAction,
        trace_address: &[usize],
        tx_hash: &H256,
        block_num: u64,
    ) -> Result<(), TraceParseError> {
        // check decoding with the regular abi

        // tries to get the proxy abi -> decode
        let proxy_abi = self.etherscan_client.proxy_contract_abi(action.to.into()).await?;

        Ok(())
    }
}
