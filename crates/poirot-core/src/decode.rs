use crate::action::Action;
use alloy_dyn_abi::{DynSolType, ResolveSolType};
use alloy_etherscan::{errors::EtherscanError, Client};
use alloy_json_abi::{JsonAbi, StateMutability};
use colored::*;

use ethers_core::types::Chain;
use log::{debug, warn};
use reth_primitives::{H256, U256};
use reth_rpc_types::trace::parity::{
    Action as RethAction, CallAction, CallType, LocalizedTransactionTrace,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Default)]
pub struct ParserStats {
    pub total_traces: usize,
    pub successful_parses: usize,
    pub not_call_action_errors: usize,
    pub empty_input_errors: usize,
    pub etherscan_errors: usize,
    pub abi_parse_errors: usize,
    pub invalid_function_selector_errors: usize,
    pub abi_decoding_failed_errors: usize,
}

impl ParserStats {
    pub fn increment_error(&mut self, error: TraceParseError) {
        match error {
            TraceParseError::NotCallAction(_) => self.not_call_action_errors += 1,
            TraceParseError::EmptyInput(_) => self.empty_input_errors += 1,
            TraceParseError::EtherscanError(_) => self.etherscan_errors += 1,
            TraceParseError::AbiParseError(_) => self.abi_parse_errors += 1,
            TraceParseError::InvalidFunctionSelector(_) => {
                self.invalid_function_selector_errors += 1
            }
            TraceParseError::AbiDecodingFailed(_) => self.abi_parse_errors += 1,
        };
    }

    pub fn increment_success(&mut self) {
        self.successful_parses += 1;
    }

    pub fn display(&self) {
        println!("{}", "Parser Statistics".bold().underline());
        println!("{}: {}", "Total Traces".green().bold(), self.total_traces.to_string().cyan());
        println!(
            "{}: {}",
            "Successful Parses".green().bold(),
            self.successful_parses.to_string().cyan()
        );
        println!(
            "{}: {}",
            "Not Call Action Errors".red().bold(),
            self.not_call_action_errors.to_string().cyan()
        );
        println!(
            "{}: {}",
            "Empty Input Errors".red().bold(),
            self.empty_input_errors.to_string().cyan()
        );
        println!(
            "{}: {}",
            "Etherscan Errors".red().bold(),
            self.etherscan_errors.to_string().cyan()
        );
        println!(
            "{}: {}",
            "ABI Parse Errors".red().bold(),
            self.abi_parse_errors.to_string().cyan()
        );
        println!(
            "{}: {}",
            "Invalid Function Selector Errors".red().bold(),
            self.invalid_function_selector_errors.to_string().cyan()
        );
        println!(
            "{}: {}",
            "ABI Decoding Failed Errors".red().bold(),
            self.abi_decoding_failed_errors.to_string().cyan()
        );
    }
}

/// A [`Parser`] will iterate through a block's Parity traces and attempt to decode each call for
/// later analysis.
pub struct Parser {
    /// Parity block traces.
    pub block_trace: Vec<LocalizedTransactionTrace>,
    /// Etherscan client for fetching ABI for each contract address.
    pub client: Client,

    pub stats: ParserStats,
}

/// Custom error type for trace parsing
#[derive(Debug)]
pub enum TraceParseError {
    NotCallAction(H256), // Added field for transaction hash
    EmptyInput(H256),    // Added field for transaction hash
    EtherscanError(EtherscanError),
    AbiParseError(serde_json::Error),
    InvalidFunctionSelector(H256),
    AbiDecodingFailed(H256)
}

impl Parser {
    /// Public constructor function to instantiate a new [`Parser`].
    /// # Arguments
    /// * `block_trace` - Block trace from [`TracingClient`].
    /// * `etherscan_key` - Etherscan API key to instantiate client
    pub fn new(block_trace: Vec<LocalizedTransactionTrace>, etherscan_key: String) -> Self {
        let paths = fs::read_dir("./").unwrap();

        for path in paths {
            println!("Name: {}", path.unwrap().path().display())
        }
        let cache_directory = "./abi_cache";

        // Check if the cache directory exists, and create it if it doesn't.
        if !Path::new(cache_directory).exists() {
            fs::create_dir_all(cache_directory).expect("Failed to create cache directory");
        }

        Self {
            block_trace,
            client: Client::new_cached(
                Chain::Mainnet,
                etherscan_key,
                Some(PathBuf::from(cache_directory)),
                std::time::Duration::new(10000, 0),
            )
            .unwrap(),
            stats: ParserStats::default(),
        }
    }

    /// Attempt to parse each trace in a block.
    pub async fn parse(&mut self) -> Vec<Action> {
        let mut result = vec![];

        for trace in &self.block_trace {
            self.stats.total_traces += 1;
            match self.parse_trace(trace).await {
                Ok(res) => {
                    self.stats.successful_parses += 1;
                    result.push(res);
                }
                Err(e) => {
                    warn!("{}", format!("Error parsing trace: {:?}", e));
                    self.stats.increment_error(e);
                }
            }
        }

        result
    }

    pub async fn parse_trace(
        &self,
        trace: &LocalizedTransactionTrace,
    ) -> Result<Action, TraceParseError> {
        let (action, _call_type) = match &trace.trace.action {
            RethAction::Call(call) => (call, &call.call_type),
            _ => return Err(TraceParseError::NotCallAction(trace.transaction_hash.unwrap())),
        };

        let abi = self
            .client
            .contract_abi(action.to.into())
            .await
            .map_err(TraceParseError::EtherscanError)?;

        // Check if the input is empty, indicating a potential `receive` or `fallback` function
        // call.
        if action.input.is_empty() {
            return handle_empty_input(&abi, action, trace)
        }

        // Decode the input based on the ABI.
        // Try to decode the input with the original ABI
        match decode_input_with_abi(&abi, action, trace) {
            Ok(Some(decoded_input)) => Ok(decoded_input),
            Ok(None) | Err(_) => {
                // If decoding with the original ABI failed, fetch the implementation ABI and try
                // again
                let impl_abi = self
                    .client
                    .delegate_raw_contract(action.to.into())
                    .await
                    .map_err(TraceParseError::EtherscanError)?;
                decode_input_with_abi(&impl_abi, action, trace)?.ok_or(
                    TraceParseError::InvalidFunctionSelector(trace.transaction_hash.unwrap()),
                )
            }
        }
    }
}

fn decode_input_with_abi(
    abi: &JsonAbi,
    action: &CallAction,
    trace: &LocalizedTransactionTrace,
) -> Result<Option<Action>, TraceParseError> {
    for functions in abi.functions.values() {
        for function in functions {
            if function.selector() == action.input[..4] {
                // Resolve all inputs
                let mut resolved_params: Vec<DynSolType> = Vec::new();
                for param in &function.inputs {
                    let _ =
                        param.resolve().map(|resolved_param| resolved_params.push(resolved_param));
                }

                let inputs = &action.input[4..]; // Remove the function selector from the input.
                let params_type = DynSolType::Tuple(resolved_params); // Construct a tuple type from the resolved parameters.

                // Decode the inputs based on the resolved parameters.
                match params_type.decode_params(inputs) {
                    Ok(decoded_params) => {
                        debug!(
                            "For function {}: Decoded params: {:?} \n, with tx hash: {:#?}",
                            function.name, decoded_params, trace.transaction_hash
                        );
                        return Ok(Some(Action::new(
                            function.name.clone(),
                            Some(decoded_params),
                            trace.clone(),
                        )))
                    }
                    Err(e) => warn!("Failed to decode params: {}", e),
                }
            }
        }
    }
    // No matching function selector was found in this ABI
    Err(TraceParseError::AbiDecodingFailed(trace.transaction_hash.unwrap()))
}

fn handle_empty_input(
    abi: &JsonAbi,
    action: &CallAction,
    trace: &LocalizedTransactionTrace,
) -> Result<Action, TraceParseError> {
    if action.value != U256::from(0) {
        if let Some(receive) = &abi.receive {
            if receive.state_mutability == StateMutability::Payable {
                return Ok(Action::new("receive".to_string(), None, trace.clone()))
            }
        }

        if let Some(fallback) = &abi.fallback {
            if fallback.state_mutability == StateMutability::Payable {
                return Ok(Action::new("fallback".to_string(), None, trace.clone()))
            }
        }
    }
    Err(TraceParseError::EmptyInput(trace.transaction_hash.unwrap()))
}
