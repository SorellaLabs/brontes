use crate::action::Action;
use alloy_dyn_abi::{DynSolType, ResolveSolType};
use alloy_etherscan::{errors::EtherscanError, Client};
use alloy_json_abi::StateMutability;
use colored::*;
use ethers::types::H160;
use ethers_core::types::Chain;
use reth_primitives::{H256, U256};
use reth_rpc_types::trace::parity::{Action as RethAction, CallType, LocalizedTransactionTrace};
use std::path::PathBuf;

/// A [`Parser`] will iterate through a block's Parity traces and attempt to decode each call for
/// later analysis.
pub struct Parser {
    /// Parity block traces.
    pub block_trace: Vec<LocalizedTransactionTrace>,
    /// Etherscan client for fetching ABI for each contract address.
    pub client: Client,
}

/// Custom error type for trace parsing
#[derive(Debug)]
pub enum TraceParseError {
    NotCallAction(H256), // Added field for transaction hash
    EmptyInput(H256),    // Added field for transaction hash
    EtherscanError(EtherscanError),
    AbiParseError(serde_json::Error),
    InvalidFunctionSelector(H256), // Added field for transaction hash
}
impl Parser {
    /// Public constructor function to instantiate a new [`Parser`].
    /// # Arguments
    /// * `block_trace` - Block trace from [`TracingClient`].
    /// * `etherscan_key` - Etherscan API key to instantiate client.
    pub fn new(block_trace: Vec<LocalizedTransactionTrace>, etherscan_key: String) -> Self {
        Self {
            block_trace,
            client: Client::new_cached(
                Chain::Mainnet,
                etherscan_key,
                Some(PathBuf::from("./abi_cache")),
                std::time::Duration::new(1000000, 0),
            )
            .unwrap(),
        }
    }

    /// Attempt to parse each trace in a block.
    pub async fn parse(&self) -> Vec<Action> {
        let mut result = vec![];

        for trace in &self.block_trace {
            match self.parse_trace(trace).await {
                Ok(res) => {
                    result.push(res);
                }
                Err(e) => {
                    eprintln!("{}", format!("Error parsing trace: {:?}", e).red());
                    continue
                }
            }
        }

        result
    }

    pub async fn parse_trace(
        &self,
        trace: &LocalizedTransactionTrace,
    ) -> Result<Action, TraceParseError> {
        let (action, call_type) = match &trace.trace.action {
            RethAction::Call(call) => (call, &call.call_type),
            _ => return Err(TraceParseError::NotCallAction(trace.transaction_hash.unwrap())),
        };

        let abi = match call_type {
            &CallType::DelegateCall => {
                // Fetch proxy implementation
                self.client
                    .delegate_raw_contract(H160(action.to.to_fixed_bytes()))
                    .await
                    .map_err(TraceParseError::EtherscanError)?
            }
            _ => {
                // For other call types, use the original method.
                self.client
                    .contract_abi(H160(action.to.to_fixed_bytes()))
                    .await
                    .map_err(TraceParseError::EtherscanError)?
            }
        };

        // Check if the input is empty, indicating a potential `receive` or `fallback` function
        // call.
        if action.input.is_empty() {
            // If a non-zero value was transferred, this is a call to the `receive` or `fallback`
            // function.
            if action.value != U256::from(0) {
                // Check if the contract has a `receive` function.
                if let Some(receive) = abi.receive {
                    // Ensure the `receive` function is payable.
                    if receive.state_mutability == StateMutability::Payable {
                        return Ok(Action::new("receive".to_string(), None, trace.clone()))
                    }
                }
                // If no `receive` function or it's not payable, check if there's a payable
                // `fallback` function.
                else if let Some(fallback) = abi.fallback {
                    if fallback.state_mutability == StateMutability::Payable {
                        return Ok(Action::new("fallback".to_string(), None, trace.clone()))
                    }
                }
            }

            return Err(TraceParseError::EmptyInput(trace.transaction_hash.unwrap()))
        }

        for functions in abi.functions.values() {
            for function in functions {
                if function.selector() == action.input[..4] {
                    // Resolve all inputs
                    let mut resolved_params: Vec<DynSolType> = Vec::new();
                    for param in &function.inputs {
                        let _ = param
                            .resolve()
                            .map(|resolved_param| resolved_params.push(resolved_param));
                    }

                    let inputs = &action.input[4..]; // Remove the function selector from the input.
                    let params_type = DynSolType::Tuple(resolved_params); // Construct a tuple type from the resolved parameters.

                    // Decode the inputs based on the resolved parameters.
                    match params_type.decode_params(inputs) {
                        Ok(decoded_params) => {
                            println!(
                                "For function {}: Decoded params: {:?} \n, with tx hash: {:#?}",
                                function.name, decoded_params, trace.transaction_hash
                            );
                            return Ok(Action::new(
                                function.name.clone(),
                                Some(decoded_params),
                                trace.clone(),
                            ))
                        }
                        Err(e) => eprintln!("Failed to decode params: {}", e),
                    }
                }
            }
        }

        Err(TraceParseError::InvalidFunctionSelector(trace.transaction_hash.unwrap()))
    }
}

//TODO: Deal with all action types, so if there is delegate call we need to fetch the
// implementation abi
