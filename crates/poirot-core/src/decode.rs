use crate::{structured_trace::{
        CallAction,
        StructuredTrace::{self, CALL, CREATE},
        TxTrace,
    }, stats::ParserStats, errors::TraceParseError};
use alloy_dyn_abi::{DynSolType, ResolveSolType};
use alloy_etherscan::{errors::EtherscanError, Client};
use alloy_json_abi::{JsonAbi, StateMutability};

use ethers_core::types::{Chain, Trace};
use reth_primitives::{H256, U256};
use reth_rpc_types::trace::parity::{
    Action as RethAction, CallAction as RethCallAction, CallType, TraceResultsWithTransactionHash,
};
use tracing::{error, instrument, span, warn, info};
use std::{
    fs,
    path::{Path, PathBuf},
};
// tracing




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
            error!("Failed to read directory: {}", err);
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
        }
    }


    #[instrument(skip(self, block_trace))]
    pub async fn parse_block(
        &mut self,
        block_num: u64,
        block_trace: Vec<TraceResultsWithTransactionHash>,
    ) -> Vec<TxTrace> {
        let mut result: Vec<TxTrace> = vec![];

        for (idx, trace) in block_trace.iter().enumerate() {
    
            match self.parse_tx(trace, idx).await {
                Ok(res) => {
                    result.push(res);
                }
                Err(error) => {
                    warn!(?error, "Error parsing trace");
                }
            }
        }
        result
    }

    #[instrument(skip(self, trace))]
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
                _ => return Err(TraceParseError::NotRecognizedAction(trace.transaction_hash.into())),
            };

            let fetch_abi_result = self.client.contract_abi(action.to.into()).await;

            let abi = match fetch_abi_result {
                Ok(a) => a,
                Err(e) => {
                    let error = TraceParseError::EtherscanError(e);
                    warn!(?error, "Failed to fetch contract ABI");
                    return Err(error);
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

                    let decoded_input = if let Some(input) = decode_input_with_abi(&impl_abi, action, &trace_address, tx_hash)? {
                        Ok(input)
                    } else{
                        let error = TraceParseError::InvalidFunctionSelector(trace.transaction_hash.into());
                        warn!(%error, "Invalid Function Selector");
                        Err(error)
                    };
                    structured_traces.push(decoded_input?);
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
                        info!(
                            "For function {}: Decoded params: {:?} \n, with tx hash: {:#?}",
                            function.name, decoded_params, tx_hash
                        );
                        return Ok(Some(StructuredTrace::CALL(CallAction::new(
                            action.from,
                            action.to,
                            function.name.clone(),
                            Some(decoded_params),
                            trace_address.clone(),
                        ))))
                    }
                    Err(e) => {
                        warn!(error=?e, "Failed to decode params");
                    },
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
                    RECEIVE.to_string(),
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
                    FALLBACK.to_string(),
                    None,
                    trace_address.clone(),
                )))
            }
        }
    }
    Err(TraceParseError::EmptyInput(tx_hash.clone().into()))
}
