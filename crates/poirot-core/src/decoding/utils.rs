use crate::{
    errors::TraceParseError,
    structured_trace::{
        CallAction,
        StructuredTrace::{self},
    },
};
extern crate reth_tracing;
use super::*;
use alloy_dyn_abi::{DynSolType, ResolveSolType};
use alloy_json_abi::{JsonAbi, StateMutability};
use alloy_sol_types::sol;
use reth_primitives::{H256, U256};
use reth_rpc_types::trace::parity::{
    Action as RethAction, CallAction as RethCallAction, TransactionTrace,
};

sol! {
    interface IDiamondLoupe {
        function facetAddress(bytes4 _functionSelector) external view returns (address facetAddress_);
    }
}

pub(crate) fn decode_input_with_abi(
    abi: &JsonAbi,
    action: &RethCallAction,
    trace_address: &[usize],
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
                //println!("deeeeeg PARAM {:?}", &resolved_params);
                let params_type = DynSolType::Tuple(resolved_params);

                // Remove the function selector from the input.
                let inputs = &action.input[4..];
                //println!("deeeeeg INPUTS {:?}", &inputs);
                // Decode the inputs based on the resolved parameters.
                match params_type.decode_params(inputs) {
                    Ok(decoded_params) => {
                        return Ok(StructuredTrace::CALL(CallAction::new(
                            action.from,
                            action.to,
                            action.value,
                            function.name.clone(),
                            Some(decoded_params),
                            trace_address.to_owned(),
                        )))
                    }
                    Err(_) => return Err(TraceParseError::AbiDecodingFailed((*tx_hash).into())),
                }
            }
        }
    }

    Err(TraceParseError::InvalidFunctionSelector((*tx_hash).into()))
}

pub(crate) fn handle_empty_input(
    abi: &JsonAbi,
    action: &RethCallAction,
    trace_address: &[usize],
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
                    trace_address.to_owned(),
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
                    trace_address.to_owned(),
                )))
            }
        }
    }
    Err(TraceParseError::EmptyInput((*tx_hash).into()))
}

/// decodes the trace action
pub(crate) fn decode_trace_action(
    structured_traces: &mut Vec<StructuredTrace>,
    transaction_trace: &TransactionTrace,
) -> Option<(RethCallAction, Vec<usize>)> {
    match &transaction_trace.action {
        RethAction::Call(call) => Some((call.clone(), transaction_trace.trace_address.clone())),
        RethAction::Create(create_action) => {
            structured_traces.push(StructuredTrace::CREATE(create_action.clone()));
            None
        }
        RethAction::Selfdestruct(self_destruct) => {
            structured_traces.push(StructuredTrace::SELFDESTRUCT(self_destruct.clone()));
            None
        }
        RethAction::Reward(reward) => {
            structured_traces.push(StructuredTrace::REWARD(reward.clone()));
            None
        }
    }
}
