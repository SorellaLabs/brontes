use crate::{
    decoding::utils::*,
    errors::TraceParseError,
    stats::types::{BlockStats, TraceStats, TransactionStats},
};
use alloy_etherscan::Client;
use alloy_json_abi::JsonAbi;
use poirot_types::structured_trace::{
    CallAction,
    StructuredTrace::{self},
};

use reth_tracing::TracingClient;

use super::{utils::IDiamondLoupe::facetAddressCall, *};
use reth_primitives::H256;
use reth_rpc_types::{
    trace::parity::{Action as RethAction, CallAction as RethCallAction},
    CallRequest,
};
use std::sync::Arc;

extern crate reth_tracing;

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
            if transaction_traces.is_none() {
                traces.push(TxTrace::new(vec![], tx_hash, idx as usize));
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
                .parse_transaction(transaction_traces.unwrap(), block_num, tx_hash, idx as u16)
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
        block_num: u64,
        tx_hash: H256,
        tx_idx: u16,
    ) -> (TxTrace, TransactionStats) {
        init_trace!(tx_hash, tx_idx, tx_trace.len());
        let mut traces = Vec::new();
        let mut stats = TransactionStats { block_num, tx_hash, tx_idx, traces: vec![], err: None };

        let len = tx_trace.len();
        for (idx, trace) in tx_trace.into_iter().enumerate() {
            let trace = self.parse_trace(trace, block_num, tx_hash).await;
            let mut stat = TraceStats::new(block_num, tx_hash, tx_idx, idx as u16, None);
            if let Err(e) = trace {
                stat.err = Some(Into::<TraceParseErrorKind>::into(&e));
            } else {
                traces.push(trace.unwrap());
            }
            stat.trace(len);
            stats.traces.push(stat);
        }

        stats.trace();
        (TxTrace::new(traces, tx_hash, tx_idx as usize), stats)
    }

    /// pushes each trace to parser_fut
    async fn parse_trace(
        &self,
        trace: TransactionTrace,
        block_num: u64,
        tx_hash: H256,
    ) -> Result<StructuredTrace, TraceParseError> {
        let (action, trace_address) = if let RethAction::Call(call) = trace.action {
            (call, trace.trace_address)
        } else {
            return Ok(decode_trace_action(&trace))
        };

        let abi = self.etherscan_client.contract_abi(action.to.into()).await?;

        // Check if the input is empty, indicating a potential `receive` or `fallback` function
        // call.
        if action.input.is_empty() {
            return handle_empty_input(&abi, &action, &trace_address, &tx_hash)
        }

        match self.abi_decoding_pipeline(&abi, &action, &trace_address, &tx_hash, block_num).await {
            Ok(s) => Ok(s),
            Err(_) => {
                return Ok(StructuredTrace::CALL(CallAction::new(
                    action.from,
                    action.to,
                    action.value,
                    UNKNOWN.to_string(),
                    None,
                    trace_address.clone(),
                )))
            }
        }

        /*
        let err: Option<TraceParseErrorKind> = if let Err(e) = &structured_trace {
            error_trace!(tx_hash, e);
            Some(e.into())
        } else {
            success_trace!(tx_hash);
            None
        };

        let res = if err.is_none() { Some(structured_trace.unwrap()) } else { None };

        let _ = self.metrics_tx.send(TraceMetricEvent::TraceMetricRecieved {
            block_num,
            tx_hash,
            tx_idx,
            tx_trace_idx: trace_idx,
            error: err.map(|e| e.into()),
        });

        res*/
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
    ) -> Result<StructuredTrace, TraceParseError> {
        // check decoding with the regular abi
        if let Ok(structured_trace) = decode_input_with_abi(&abi, &action, &trace_address, &tx_hash)
        {
            return Ok(structured_trace)
        };

        // tries to get the proxy abi -> decode
        let proxy_abi = self.etherscan_client.proxy_contract_abi(action.to.into()).await?;
        if let Ok(structured_trace) =
            decode_input_with_abi(&proxy_abi, &action, &trace_address, &tx_hash)
        {
            return Ok(structured_trace)
        };

        // tries to decode with the new abi
        // if unsuccessful, returns an error
        let diamond_proxy_abi = self.diamond_proxy_contract_abi(&action, block_num).await?;
        if let Ok(structured_trace) =
            decode_input_with_abi(&diamond_proxy_abi, &action, &trace_address, &tx_hash)
        {
            return Ok(structured_trace)
        };

        Err(TraceParseError::AbiDecodingFailed(tx_hash.clone().into()))
    }

    /// retrieves the abi from a possible diamond proxy contract
    async fn diamond_proxy_contract_abi(
        &self,
        action: &RethCallAction,
        block_num: u64,
    ) -> Result<JsonAbi, TraceParseError> {
        let diamond_call =
            // TODO: why _ ?
            facetAddressCall { _functionSelector: action.input[..4].try_into().unwrap() };

        let call_data = diamond_call.encode();

        let call_request =
            CallRequest { to: Some(action.to), data: Some(call_data.into()), ..Default::default() };

        let data: Bytes = self
            .tracer
            .api
            .call(call_request, Some(block_num.into()), EvmOverrides::default())
            .await
            .map_err(|e| Into::<TraceParseError>::into(e))?;

        let facet_address = facetAddressCall::decode_returns(&data, true).unwrap().facetAddress_;

        let abi = self
            .etherscan_client
            .contract_abi(facet_address.into_array().into())
            .await
            .map_err(|e| Into::<TraceParseError>::into(e))?;

        Ok(abi)
    }
}
