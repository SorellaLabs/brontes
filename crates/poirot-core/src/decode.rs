use crate::{
    errors::TraceParseError,
    stats::ParserStats,
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
use tracing::{error, info, instrument, span, warn};
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

    // Should parse all transactions, if a tx fails to parse it should still be stored with None
    // fields on the decoded subfield

    #[instrument(skip(self, block_trace))]
    pub async fn parse_block(
        &mut self,
        block_num: u64,
        block_trace: Vec<TraceResultsWithTransactionHash>,
    ) -> Vec<TxTrace> {
        let mut result: Vec<TxTrace> = vec![];

        for (idx, trace) in block_trace.iter().enumerate() {
            //We don't need to through an error for this given transaction so long as the error is
            // logged & emmitted and the transaction is stored.
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

    // TODO: Then figure out how to deal with error
    // TODO: need to add decoding for diamond proxy

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
                RethAction::Selfdestruct(self_destruct) => {
                    structured_traces.push(StructuredTrace::SELFDESTRUCT(self_destruct.clone()));
                    continue
                }
                RethAction::Reward(reward) => {
                    structured_traces.push(StructuredTrace::REWARD(reward.clone()));
                    continue
                }
            };

            println!("action ----------------------------------------- {:?}", action);


            let abi = match self.client.contract_abi(action.to.into()).await {
                Ok(a) => a,
                Err(e) => {
                    warn!(error=?TraceParseError::EtherscanError(e), "Failed to fetch contract ABI");
                    continue
                }
            };

            // Check if the input is empty, indicating a potential `receive` or `fallback` function
            // call.
            println!("{:?}" action.input.is_empty());
            if action.input.is_empty() {
                match handle_empty_input(&abi, action, &trace_address, tx_hash) {
                    Ok(structured_trace) => {
                        structured_traces.push(structured_trace);
                    }
                    Err(error) => {
                        warn!(?error, "Empty Input without fallback or receive");
                        continue
                    }
                }
            }

            // Decode the input based on the ABI.
            // If the decoding fails, you have to make a call to:
            // facetAddress(function selector) which is a function on any diamond proxy contract, if
            // it returns it will give you the address of the facet which can be used to
            // fetch the ABI Use the sol macro to previously generate the facetAddress
            // function binding & call it on the to address that is being called in the first place https://docs.rs/alloy-sol-macro/latest/alloy_sol_macro/macro.sol.html
            // you will have to do a call on the reth_api which you have to add to the reth_tracing
            // crate lib.rs. Just copy how it is done in the ethers-reth repo

            let structure_input = match decode_input_with_abi(&abi, action, &trace_address, tx_hash)
            {
                Ok(d) => d,
                Err(e) => {
                    // If decoding with the original ABI failed, fetch the implementation ABI and
                    // try again
                    let impl_abi = match self
                        .client
                        .proxy_contract_abi(action.to.into())
                        .await {
                            Ok(abi) => abi,
                            Err(e) => {
                                warn!(error=?TraceParseError::EtherscanError(e), "unable to get proxy contract abi");
                                continue;
                            }
                        };

                    match decode_input_with_abi(&impl_abi, action, &trace_address, tx_hash) {
                        Ok(s) => s,
                        Err(error) => {
                            warn!(?error, "Invalid Function Selector");
                            StructuredTrace::CALL(CallAction::new(
                                action.from,
                                action.to,
                                action.value,
                                UNKNOWN.to_string(),
                                None,
                                trace_address.clone(),
                            ))
                        }
                    }
                }
            };
            structured_traces.push(structure_input);
        }

        Ok(TxTrace { trace: structured_traces, tx_hash: trace.transaction_hash, tx_index })
    }
}

fn decode_input_with_abi(
    abi: &JsonAbi,
    action: &RethCallAction,
    trace_address: &Vec<usize>,
    tx_hash: &H256,
) -> Result<StructuredTrace, TraceParseError> {
    for functions in abi.functions.values() {
        for function in functions {
            if function.selector() == action.input[..4] {
                // Resolve all inputs
                let mut resolved_params: Vec<DynSolType> = Vec::new();
                // TODO: Figure out how we could get an error & how to handle
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
                            "Function: {}\nDecoded params: {:?}\nTx hash: {:#?}",
                            function.name, decoded_params, tx_hash
                        );
                        return Ok(StructuredTrace::CALL(CallAction::new(
                            action.from,
                            action.to,
                            action.value,
                            function.name.clone(),
                            Some(decoded_params),
                            trace_address.clone(),
                        )))
                    }
                    Err(e) => {
                        return Err(TraceParseError::AbiDecodingFailed(tx_hash.clone().into()))
                    }
                }
            }
        }
    }
    return Err(TraceParseError::InvalidFunctionSelector(tx_hash.clone().into()))
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
                    action.value,
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
                    action.value,
                    FALLBACK.to_string(),
                    None,
                    trace_address.clone(),
                )))
            }
        }
    }
    Err(TraceParseError::EmptyInput(tx_hash.clone().into()))
}
