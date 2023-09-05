use crate::{
    decoding::utils::*,
    errors::TraceParseError,
    structured_trace::{
        CallAction,
        StructuredTrace::{self},
    },
};
use alloy_etherscan::Client;
use alloy_json_abi::JsonAbi;

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

/// A [`TraceParser`] will iterate through a block's Parity traces and attempt to decode each call for
/// later analysis.
pub struct TraceParser {
    pub etherscan_client: Client,
    pub tracer: Arc<TracingClient>,
}

impl TraceParser {
    pub fn new(etherscan_client: Client, tracer: Arc<TracingClient>) -> Self {
        Self { etherscan_client, tracer }
    }

    /// parses a trace in a tx
    pub(crate) async fn parse(
        &self,
        trace: TransactionTrace,
        tx_hash: H256,
        block_num: u64,
    ) -> Result<StructuredTrace, TraceParseError> {
        //init_trace!(tx_hash, idx, transaction_traces.len());

        // TODO: we can use the let else caluse here instead of if let else
        let (action, trace_address) = if let RethAction::Call(call) = trace.action {
            (call, trace.trace_address)
        } else {
            return Ok(decode_trace_action(&trace));
        };

        let abi = self.etherscan_client.contract_abi(action.to.into()).await?;

        // Check if the input is empty, indicating a potential `receive` or `fallback` function
        // call.
        if action.input.is_empty() {
            return handle_empty_input(&abi, &action, &trace_address, &tx_hash);
        }

        match self.abi_decoding_pipeline(&abi, &action, &trace_address, &tx_hash, block_num).await {
            Ok(s) => return Ok(s),
            Err(_) => {
                return Ok(StructuredTrace::CALL(CallAction::new(
                    action.from,
                    action.to,
                    action.value,
                    UNKNOWN.to_string(),
                    None,
                    trace_address.clone(),
                )));
            }
        };
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
            return Ok(structured_trace);
        };

        // tries to get the proxy abi -> decode
        let proxy_abi = self.etherscan_client.proxy_contract_abi(action.to.into()).await?;
        if let Ok(structured_trace) =
            decode_input_with_abi(&proxy_abi, &action, &trace_address, &tx_hash)
        {
            return Ok(structured_trace);
        };

        // tries to decode with the new abi
        // if unsuccessful, returns an error
        let diamond_proxy_abi = self.diamond_proxy_contract_abi(&action, block_num).await?;
        if let Ok(structured_trace) =
            decode_input_with_abi(&diamond_proxy_abi, &action, &trace_address, &tx_hash)
        {
            return Ok(structured_trace);
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
