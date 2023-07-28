use crate::{
    errors::TraceParseError,
    structured_trace::{
        StructuredTrace::{self},
        TxTrace, CallAction,
    },
    *,
};
use alloy_dyn_abi::{DynSolType, ResolveSolType};
use alloy_etherscan::{Client, errors::EtherscanError};
use alloy_json_abi::{JsonAbi, StateMutability};
use colored::Colorize;

use ethers_core::{types::Chain, abi::Address};
use reth_primitives::{H256, U256};
use reth_rpc_types::trace::parity::{
    Action as RethAction, CallAction as RethCallAction, TraceResultsWithTransactionHash, ActionType, TransactionTrace,
};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tracing::{error, info, instrument};

use super::*;


/// cycles through all possible abi decodings
/// 1) regular
/// 2) proxy
/// 3) diamond proxy
pub(crate) async fn abi_decoding_pipeline(    
    client: &Client,
    abi: &JsonAbi,
    action: &RethCallAction,
    trace_address: &[usize],
    tx_hash: &H256
) -> Result<StructuredTrace, TraceParseError> {

    // check decoding with the regular abi
    if let Ok(structured_trace) = decode_input_with_abi(&abi, &action, &trace_address, &tx_hash) {
        return Ok(structured_trace)
    };

    // tries to get the proxy abi, 
    // if unsuccessful, tries to get the diamond proxy abi
    let proxy_abi = match client.proxy_contract_abi(action.to.into()).await {
        Ok(abi) => abi,
        Err(e) => diamond_proxy_contract_abi(&client, action.to.into()).await?
    };

    // tries to decode with the new abi
    // if unsuccessful, returns an error
    decode_input_with_abi(&proxy_abi, &action, &trace_address, &tx_hash)
}


pub(crate) async fn diamond_proxy_contract_abi(client: &Client, address: Address) -> Result<JsonAbi, EtherscanError> {
    let contract_metadata = client.contract_source_code(address).await?;

    //println!("{:?}", contract_metadata);
    // Use the first item in the metadata.
    let first_item = &contract_metadata.items[0];

    // If the first item is a proxy, get its implementation address and fetch the ABI.
    let implementation_address = match first_item.implementation {
        Some(impl_addr) => impl_addr,
        None => return Err(EtherscanError::MissingImplementationAddress),
    };

    // Get the ABI of the implementation contract.
    client.contract_abi(implementation_address).await
}



pub(crate) fn decode_input_with_abi(
    abi: &JsonAbi,
    action: &RethCallAction,
    trace_address: &[usize],
    tx_hash: &H256,
) -> Result<StructuredTrace, TraceParseError> {
    for functions in abi.functions.values() {
        for function in functions {
            println!("\ndeeeeeg FS {:?}", function.selector());
            println!("deeeeeg FI {:?}", &function.inputs);
            if function.selector() == action.input[..4] {
                // Resolve all inputs
                let mut resolved_params: Vec<DynSolType> = Vec::new();
                // TODO: Figure out how we could get an error & how to handle
                for param in &function.inputs {
                    let _ =
                        param.resolve().map(|resolved_param| resolved_params.push(resolved_param));
                }
                println!("deeeeeg PARAM {:?}", &resolved_params);
                let params_type = DynSolType::Tuple(resolved_params);

                // Remove the function selector from the input.
                let inputs = &action.input[4..];
                println!("deeeeeg INPUTS {:?}", &inputs);
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

    println!("deeeeeg ABI {:?}", abi);
    println!("deeeeeg ABI FUNC VALS {:?}", abi.functions.values());
    println!("deeeeeg ACTION {:?}\n", action);

    

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
                success_trace!(
                    tx_hash,
                    trace_action = "CALL",
                    call_type = format!("{:?}", action.call_type)
                );
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
                success_trace!(
                    tx_hash,
                    trace_action = "CALL",
                    call_type = format!("{:?}", action.call_type)
                );
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
pub(crate) fn decode_trace_action(structured_traces: &mut Vec<StructuredTrace>, transaction_trace: &TransactionTrace, tx_hash: &H256) -> Option<(RethCallAction, Vec<usize>)> {
    match &transaction_trace.action {
        RethAction::Call(call) => Some((call.clone(), transaction_trace.trace_address.clone())),
        RethAction::Create(create_action) => {
            success_trace!(
                tx_hash,
                trace_action = "CREATE",
                creator_addr = format!("{:#x}", create_action.from)
            );
            structured_traces.push(StructuredTrace::CREATE(create_action.clone()));
            None
        }
        RethAction::Selfdestruct(self_destruct) => {
            success_trace!(
                tx_hash,
                trace_action = "SELFDESTRUCT",
                contract_addr = format!("{:#x}", self_destruct.address)
            );
            structured_traces.push(StructuredTrace::SELFDESTRUCT(self_destruct.clone()));
            None
        }
        RethAction::Reward(reward) => {
            success_trace!(
                tx_hash,
                trace_action = "REWARD",
                reward_type = format!("{:?}", reward.reward_type),
                reward_author = format!("{:#x}", reward.author)
            );
            structured_traces.push(StructuredTrace::REWARD(reward.clone()));
            None
        }
    }

}