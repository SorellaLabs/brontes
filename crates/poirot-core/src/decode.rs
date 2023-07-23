use crate::{
    parser_stats::{
        ParserStats,
        TraceParseError::{self, *},
    },
    structured_trace::{
        CallAction,
        StructuredTrace::{self, CALL, CREATE},
        TxTrace,
    },
};
use alloy_dyn_abi::{DynSolType, ResolveSolType};
use alloy_etherscan::{errors::EtherscanError, Client};
use alloy_json_abi::{JsonAbi, StateMutability};

use ethers_core::types::{Chain, Trace};
use reth_primitives::{H256, U256};
use reth_rpc_types::trace::parity::{
    Action as RethAction, CallAction as RethCallAction, CallType, TraceResultsWithTransactionHash,
};
use std::{
    fs,
    path::{Path, PathBuf},
};
// tracing
use tracing::{info, warn, instrument, error, span, Level, field};
use tracing::field::debug;



const UNKNOWN: &str = "unknown";
const RECEIVE: &str = "receive";
const FALLBACK: &str = "fallback";
const CACHE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10_000);




/// A [`Parser`] will iterate through a block's Parity traces and attempt to decode each call for
/// later analysis.
#[derive(Debug)]
pub struct Parser {
    pub client: Client,
}

impl Parser {
    pub fn new(etherscan_key: String) -> Self {
        let paths = fs::read_dir("./").unwrap();

        let paths = fs::read_dir("./").unwrap_or_else(|err| {
            tracing::error!("Failed to read directory: {}", err);
            std::process::exit(1);
        });

        let cache_directory = "./abi_cache";

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
            stats_history: vec![],
        }
    }


    #[instrument]
    pub async fn parse_block(
        &mut self,
        block_trace: Vec<TraceResultsWithTransactionHash>,
    ) -> Vec<TxTrace> {
        let mut result: Vec<TxTrace> = vec![];
        let mut stats = ParserStats::default();

        for (idx, trace) in block_trace.iter().enumerate() {
            let span = span!(
                Level::INFO,
                "parse_block",
                total_tx = idx + 1
            );
            let _enter = span.enter();
    
            match self.parse_tx(trace, idx).await {
                Ok(res) => {
                    result.push(res);
                }
                Err(e) => {
                    warn!(error = %e, "Error parsing trace");
                    stats.increment_error(e);
                }
            }
        }
        result
    }

    #[instrument]
    pub async fn parse_tx(
        &self,
        trace: &TraceResultsWithTransactionHash,
        tx_index: usize,
    ) -> Result<TxTrace, TraceParseError> {
        let transaction_traces =
            trace.full_trace.trace.as_ref().ok_or(TraceParseError::TraceMissing)?;

        let mut structured_traces = Vec::new();
        let tx_hash = &trace.transaction_hash;

        for transaction_trace in transaction_traces {
            let (action, trace_address) = match &transaction_trace.action {
                RethAction::Call(call) => (call, transaction_trace.trace_address.clone()),
                RethAction::Create(create_action) => {
                    structured_traces.push(StructuredTrace::CREATE(create_action.clone()));
                    continue
                }
                _ => return Err(TraceParseError::NotRecognizedAction(trace.transaction_hash)),
            };

            let fetch_abi_result = self.client.contract_abi(action.to.into()).await;

            let abi = match fetch_abi_result {
                Ok(a) => a,
                Err(EtherscanError::ContractCodeNotVerified(_)) => {
                    // If the contract is unverified, register it as unknown and proceed.
                    stats.increment_error(TraceParseError::EtherscanError(
                        EtherscanError::ContractCodeNotVerified(action.to.into()),
                    ));
                    structured_traces.push(StructuredTrace::CALL(CallAction::new(
                        action.from,
                        action.to,
                        UNKNOWN, // mark function name as unknown
                        None,                  // no inputs
                        trace_address.clone(),
                    )));
                    continue
                }
                Err(e) => {
                    let trace_error = TraceParseError::EtherscanError(e);
                    error!(error = %trace_error, "Failed to fetch contract ABI");
                    return Err(trace_error);
                }
                
            };

            // Check if the input is empty, indicating a potential `receive` or `fallback` function
            // call.
            if action.input.is_empty() {
                let structured_trace = handle_empty_input(&abi, action, &trace_address, tx_hash)?;
                structured_traces.push(structured_trace);
                continue
            }

            // Decode the input based on the ABI.
            match decode_input_with_abi(&abi, action, &trace_address, tx_hash) {
                Ok(Some(decoded_input)) => {
                    structured_traces.push(decoded_input);
                    continue
                }
                Ok(None) | Err(_) => {
                    // If decoding with the original ABI failed, fetch the implementation ABI and
                    // try again
                    let impl_abi = self
                        .client
                        .proxy_contract_abi(action.to.into())
                        .await
                        .map_err(TraceParseError::EtherscanError)?;

                    let decoded_input =
                        decode_input_with_abi(&impl_abi, action, &trace_address, tx_hash)?.ok_or(
                            TraceParseError::InvalidFunctionSelector(trace.transaction_hash),
                        )?;
                    structured_traces.push(decoded_input);
                }
            }
        }

        Ok(TxTrace { trace: structured_traces, tx_hash: trace.transaction_hash, tx_index })
    }
}

fn decode_input_with_abi(
    abi: &JsonAbi,
    action: &RethCallAction,
    trace_address: &Vec<usize>,
    tx_hash: &H256,
) -> Result<Option<StructuredTrace>, TraceParseError> {
    for functions in abi.functions.values() {
        for function in functions {
            if function.selector() == action.input[..4] {
                // Resolve all inputs
                let mut resolved_params: Vec<DynSolType> = Vec::new();
                for param in &function.inputs {
                    let _ =
                        param.resolve().map(|resolved_param| resolved_params.push(resolved_param));
                }
                let params_type = DynSolType::Tuple(resolved_params);

                // Remove the function selector from the input.
                let inputs = &action.input[4..];
                // Decode the inputs based on the resolved parameters.
                match params_type.decode_params(inputs) {
                    Ok(decoded_params) => {
                        log!(
                            "For function {}: Decoded params: {:?} \n, with tx hash: {:#?}",
                            function.name, decoded_params, tx_hash
                        );
                        return Ok(Some(StructuredTrace::CALL(CallAction::new(
                            action.from,
                            action.to,
                            &function.name,
                            Some(decoded_params),
                            trace_address.clone(),
                        ))))
                    }
                    Err(e) => warn!("Failed to decode params: {}", e),
                }
            }
        } 

    }
    Ok(None)
}

fn handle_empty_input(
    abi: &JsonAbi,
    action: &RethCallAction,
    trace_address: &Vec<usize>,
    tx_hash: &H256,
) -> Result<StructuredTrace, TraceParseError> {
    if action.value != U256::from(0) {
        if let Some(receive) = &abi.receive {
            if receive.state_mutability == StateMutability::Payable {
                return Ok(StructuredTrace::CALL(CallAction::new(
                    action.to,
                    action.from,
                    RECEIVE,
                    None,
                    trace_address.clone(),
                )))
            }
        }

        if let Some(fallback) = &abi.fallback {
            if fallback.state_mutability == StateMutability::Payable {
                return Ok(StructuredTrace::CALL(CallAction::new(
                    action.from,
                    action.to,
                    FALLBACK,
                    None,
                    trace_address.clone(),
                )))
            }
        }
    }
    Err(TraceParseError::EmptyInput(tx_hash.clone()))
}
