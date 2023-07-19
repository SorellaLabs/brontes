use crate::{
    action::{
        CallAction, ProtocolType,
        StructuredTrace::{self, CALL, CREATE},
    },
    normalize::Structure,
    parser_stats::{
        ParserStats,
        TraceParseError::{self, *},
    },
};
use alloy_dyn_abi::{DynSolType, ResolveSolType};
use alloy_etherscan::{errors::EtherscanError, Client};
use alloy_json_abi::{JsonAbi, StateMutability};

use ethers_core::types::Chain;
use log::{debug, warn};
use reth_primitives::{H256, U256};
use reth_rpc_types::trace::parity::{
    Action as RethAction, CallAction as RethCallAction, CallType, LocalizedTransactionTrace,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// A [`Parser`] will iterate through a block's Parity traces and attempt to decode each call for
/// later analysis.
pub struct Parser {
    pub client: Client,
    pub stats_history: Vec<ParserStats>,
}

impl Parser {
    /// Public constructor function to instantiate a new [`Parser`].
    /// # Arguments
    /// * `block_trace` - Block trace from [`TracingClient`].
    /// * `etherscan_key` - Etherscan API key to instantiate client
    pub fn new(etherscan_key: String) -> Self {
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
            client: Client::new_cached(
                Chain::Mainnet,
                etherscan_key,
                Some(PathBuf::from(cache_directory)),
                std::time::Duration::new(10000, 0),
            )
            .unwrap(),
            stats_history: vec![],
        }
    }

    //TODO: We don't wan't to be parsing individual traces, we want to group traces by tx hash
    //TODO: because we can infer a lot by knowing the subsequent traces & grouping them

    pub async fn parse(
        &mut self,
        block_trace: Vec<LocalizedTransactionTrace>,
    ) -> Vec<StructuredTrace> {
        let mut result = vec![];
        let mut stats = ParserStats::default();

        for trace in &block_trace {
            stats.total_traces += 1;
            match self.parse_trace(trace).await {
                Ok(res) => {
                    stats.successful_parses += 1;
                    result.push(res);
                }
                Err(e) => {
                    warn!("{}", format!("Error parsing trace: {:?}", e));
                    stats.increment_error(e);
                }
            }
        }

        self.stats_history.push(stats);
        result
    }

    pub async fn parse_trace(
        &self,
        trace: &LocalizedTransactionTrace,
    ) -> Result<StructuredTrace, TraceParseError> {
        let (action, _call_type) = match &trace.trace.action {
            RethAction::Call(call) => (call, &call.call_type),
            RethAction::Create(create_action) => {
                return Ok(StructuredTrace::CREATE(create_action.clone()))
            }
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
                    .proxy_contract_abi(action.to.into())
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
    action: &RethCallAction,
    trace: &LocalizedTransactionTrace,
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

                let inputs = &action.input[4..]; // Remove the function selector from the input.
                let params_type = DynSolType::Tuple(resolved_params); // Construct a tuple type from the resolved parameters.

                // Decode the inputs based on the resolved parameters.
                match params_type.decode_params(inputs) {
                    Ok(decoded_params) => {
                        debug!(
                            "For function {}: Decoded params: {:?} \n, with tx hash: {:#?}",
                            function.name, decoded_params, trace.transaction_hash
                        );
                        return Ok(Some(StructuredTrace::CALL(CallAction::new(
                            function.name.clone(),
                            Some(decoded_params),
                            trace.clone(),
                        ))))
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
    action: &RethCallAction,
    trace: &LocalizedTransactionTrace,
) -> Result<StructuredTrace, TraceParseError> {
    if action.value != U256::from(0) {
        if let Some(receive) = &abi.receive {
            if receive.state_mutability == StateMutability::Payable {
                return Ok(StructuredTrace::CALL(CallAction::new(
                    "receive".to_string(),
                    None,
                    trace.clone(),
                )))
            }
        }

        if let Some(fallback) = &abi.fallback {
            if fallback.state_mutability == StateMutability::Payable {
                return Ok(StructuredTrace::CALL(CallAction::new(
                    "fallback".to_string(),
                    None,
                    trace.clone(),
                )))
            }
        }
    }
    Err(TraceParseError::EmptyInput(trace.transaction_hash.unwrap()))
}
