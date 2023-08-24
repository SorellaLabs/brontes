use crate::{
    decoding::utils::*,
    errors::TraceParseError,
    stats::TraceMetricEvent,
    structured_trace::{
        CallAction,
        StructuredTrace::{self},
        TxTrace,
    },
    *,
};
use alloy_etherscan::Client;
use alloy_json_abi::JsonAbi;
use colored::Colorize;
use reth_tracing::TracingClient;
use tokio::sync::mpsc::UnboundedSender;

use super::{utils::IDiamondLoupe::facetAddressCall, *};
use ethers_core::types::Chain;
use reth_primitives::{BlockId, BlockNumberOrTag, H256};
use reth_rpc_types::{
    trace::parity::{CallAction as RethCallAction, TraceResultsWithTransactionHash, TraceType},
    CallRequest,
};
use std::{
    collections::HashSet,
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use tracing::{error, info, instrument};

extern crate reth_tracing;

use alloy_sol_types::SolCall;
use reth_primitives::Bytes;

use reth_rpc::eth::revm_utils::EvmOverrides;

/// A [`Parser`] will iterate through a block's Parity traces and attempt to decode each call for
/// later analysis.
pub struct Parser {
    pub client: Client,
    pub tracer: TracingClient,
    pub metrics_tx: Arc<UnboundedSender<TraceMetricEvent>>,
}

impl Parser {
    pub fn new(
        etherscan_key: String,
        tracer: TracingClient,
        metrics_tx: UnboundedSender<TraceMetricEvent>,
    ) -> Self {
        // TODO: tf is the double check in a dir we know exists?
        let _paths = fs::read_dir("./").unwrap();

        let _paths = fs::read_dir("./").unwrap_or_else(|err| {
            error!("Failed to read directory: {}", err);
            std::process::exit(1);
        });

        let cache_directory = "./abi_cache";

        // TODO: create dir all only creates if not exists. this is redundamt check
        // Check if the cache directory exists, and create it if it doesn't.
        if !Path::new(cache_directory).exists() {
            fs::create_dir_all(cache_directory).expect("Failed to create cache directory");
        }

        Self {
            client: Client::new_cached(
                Chain::Mainnet,
                etherscan_key,
                Some(PathBuf::from(cache_directory)),
                CACHE_TIMEOUT,
            )
            .unwrap(),
            tracer,
            metrics_tx: Arc::new(metrics_tx),
        }
    }

    /// traces a block into a vec of tx traces
    pub async fn trace_block(
        &self,
        block_number: u64,
    ) -> Result<Vec<TraceResultsWithTransactionHash>, Box<dyn Error>> {
        let mut trace_type = HashSet::new();
        trace_type.insert(TraceType::Trace);

        let parity_trace = self
            .tracer
            .trace
            .replay_block_transactions(
                BlockId::Number(BlockNumberOrTag::Number(block_number)),
                trace_type,
            )
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error>)?
            .unwrap();

        Ok(parity_trace)
    }

    // Should parse all transactions, if a tx fails to parse it should still be stored with None
    // fields on the decoded subfield
    #[instrument(skip(self, block_trace))]
    pub async fn parse_block(
        &mut self,
        block_num: u64,
        block_trace: Vec<TraceResultsWithTransactionHash>,
    ) -> Vec<TxTrace> {
        // allocate vector for specific size needed

        let mut result: Vec<TxTrace> = vec![];

        // TODO: this can be converted into a filter map and then we don't have to
        // move the results into a new vector;
        for (idx, trace) in block_trace.iter().enumerate() {
            // We don't need to through an error for this given transaction so long as the error is
            // logged & emmitted and the transaction is stored.
            init_tx!(trace.transaction_hash, idx, block_trace.len());
            match self.parse_tx(trace, idx, block_num).await {
                Ok(res) => {
                    success_tx!(block_num, trace.transaction_hash);
                    result.push(res);
                }
                Err(e) => {
                    let error: &(dyn std::error::Error + 'static) = &e;
                    error!(error, "Error Parsing Transaction {:#x}", trace.transaction_hash);
                }
            }
        }
        success_block!(block_num);

        result
    }

    /// parses the traces in a tx
    pub async fn parse_tx(
        &self,
        trace: &TraceResultsWithTransactionHash,
        tx_index: usize,
        block_num: u64,
    ) -> Result<TxTrace, TraceParseError> {
        let transaction_traces =
            trace.full_trace.trace.as_ref().ok_or(TraceParseError::TraceMissing)?;

        let mut structured_traces = Vec::new();
        let tx_hash = &trace.transaction_hash;

        for (idx, transaction_trace) in transaction_traces.iter().enumerate() {
            init_trace!(tx_hash, idx, transaction_traces.len());

            // TODO: we can use the let else caluse here instead of if let else
            let (action, trace_address) = if let Some((a, t)) =
                decode_trace_action(&mut structured_traces, &transaction_trace)
            {
                (a, t)
            } else {
                continue
            };

            let abi = match self.client.contract_abi(action.to.into()).await {
                Ok(a) => a,
                Err(e) => {
                    self.trace_result(
                        block_num,
                        tx_hash,
                        tx_index,
                        idx,
                        Some(TraceParseError::from(e)),
                        None,
                    )?;
                    continue
                }
            };

            // Check if the input is empty, indicating a potential `receive` or `fallback` function
            // call.
            if action.input.is_empty() {
                match handle_empty_input(&abi, &action, &trace_address, tx_hash) {
                    Ok(structured_trace) => {
                        structured_traces.push(structured_trace);
                        self.trace_result(
                            block_num,
                            tx_hash,
                            tx_index,
                            idx,
                            None,
                            Some(vec![("Trace Action", &format!("{:?}", action.call_type))]),
                        )?;
                        continue
                    }
                    Err(e) => {
                        self.trace_result(
                            block_num,
                            tx_hash,
                            tx_index,
                            idx,
                            Some(TraceParseError::from(e)),
                            None,
                        )?;
                        continue
                    }
                }
            }

            match self
                .abi_decoding_pipeline(&abi, &action, &trace_address, &tx_hash, block_num)
                .await
            {
                Ok(s) => {
                    self.trace_result(
                        block_num,
                        tx_hash,
                        tx_index,
                        idx,
                        None,
                        Some(vec![("Trace Action", &format!("{:?}", action.call_type))]),
                    )?;
                    structured_traces.push(s);
                }
                Err(e) => {
                    self.trace_result(
                        block_num,
                        tx_hash,
                        tx_index,
                        idx,
                        Some(TraceParseError::from(e)),
                        None,
                    )?;
                    structured_traces.push(StructuredTrace::CALL(CallAction::new(
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

        Ok(TxTrace { trace: structured_traces, tx_hash: trace.transaction_hash, tx_index })
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
        let proxy_abi = self.client.proxy_contract_abi(action.to.into()).await?;
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
            .client
            .contract_abi(facet_address.into_array().into())
            .await
            .map_err(|e| Into::<TraceParseError>::into(e))?;

        Ok(abi)
    }

    /// sends the trace result to prometheus
    fn trace_result(
        &self,
        block_num: u64,
        tx_hash: &H256,
        tx_idx: usize,
        tx_trace_idx: usize,
        error: Option<TraceParseError>,
        extra_fields: Option<Vec<(&str, &str)>>,
    ) -> Result<(), TraceParseError> {

        if let Some(err) = &error {
            error_trace!(tx_hash, err, vec = extra_fields.unwrap_or_else(Vec::new));
        } else {
            success_trace!(tx_hash, vec = extra_fields.unwrap_or_else(Vec::new));
        }

        self.metrics_tx
        .send(TraceMetricEvent::TraceMetricRecieved {
            block_num,
            tx_hash: *tx_hash,
            tx_idx: tx_idx as u64,
            tx_trace_idx: tx_trace_idx as u64,
            error: error.map(|e| e.into()),
        })
        .map_err(|e| TraceParseError::ChannelSendError(e.to_string()))?;

        Ok(())
    }
}
