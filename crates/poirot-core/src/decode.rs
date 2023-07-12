use alloy_etherscan::{Client, errors::EtherscanError};
use ethers::abi::ParamType;
use ethers_core::types::Chain;

use crate::action::Action;
use ethers::types::H160;
use reth_rpc_types::trace::parity::{Action as RethAction, LocalizedTransactionTrace};
use std::path::PathBuf;

use alloy_dyn_abi::{resolve, DynSolType, DynSolValue, ResolveSolType};
use alloy_json_abi::JsonAbi;
use ethers::abi::Abi;
use serde_json::{from_str, to_string};
use colored::*;


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
    NotCallAction,
    EmptyInput,
    EtherscanError(EtherscanError),
    AbiParseError(serde_json::Error),
    InvalidFunctionSelector,
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
                std::time::Duration::new(5, 0),
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
                    continue;
                }
            }
        }
    
        result
    }

    pub async fn parse_trace(
        &self,
        trace: &LocalizedTransactionTrace,
    ) -> Result<Action, TraceParseError> {
        // We only care about "Call" traces, so we extract them here.
        let action = match &trace.trace.action {
            RethAction::Call(call) => call,
            _ => return Err(TraceParseError::NotCallAction),
        };

        // We cannot decode a call for which calldata is zero.
        if action.input.is_empty() {
            return Err(TraceParseError::EmptyInput)
        }

        // Attempt to fetch the contract ABI from etherscan.
        let abi_json_string = self
            .client
            .raw_contract(H160(action.to.to_fixed_bytes()))
            .await
            .map_err(TraceParseError::EtherscanError)?;

        let abi: JsonAbi =
            serde_json::from_str(&abi_json_string).map_err(TraceParseError::AbiParseError)?;

        for functions in abi.functions.values() {
            for function in functions {
                if function.selector() == &action.input[..4] {
                    // Resolve all inputs
                    let mut resolved_params: Vec<DynSolType> = Vec::new();
                    for param in &function.inputs {
                        let _ = param.resolve().map(|resolved_param| resolved_params.push(resolved_param));
                    }
                    let inputs = &action.input[4..]; // Remove the function selector from the input.
                    let params_type = DynSolType::Tuple(resolved_params); // Construct a tuple type from the resolved parameters.

                    // Decode the inputs based on the resolved parameters.
                    match params_type.decode_params(inputs) {
                        Ok(decoded_params) => {
                            println!(
                                "For function {}: Decoded params: {:?} \n",
                                function.name, decoded_params
                            );
                            return Ok(Action::new(function.name.clone(), decoded_params, trace.clone()))
                        }
                        Err(e) => eprintln!("Failed to decode params: {:?}", e),
                    }


                    
                }
            }
        }

        Err(TraceParseError::InvalidFunctionSelector)
    }
}

//TODO: Get bettor error handling so that we can return etherscan related errors.
//TODO: Deal with all action types, so if there is delegate call we need to fetch the
// implementation abi
