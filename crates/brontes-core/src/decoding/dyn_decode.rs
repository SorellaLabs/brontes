use std::{collections::HashSet, path::PathBuf, pin::Pin, sync::Arc};

use alloy_dyn_abi::*;
use alloy_etherscan::Client;
use alloy_json_abi::JsonAbi;
use brontes_types::structured_trace::TxTrace;
use ethers::prelude::{Http, Middleware, Provider};
use ethers_core::types::Chain;
use ethers_reth::type_conversions::{ToEthers, ToReth};
use futures::Future;
use reth_interfaces::RethError;
use reth_primitives::{BlockId, BlockNumber, BlockNumberOrTag, Header, H256};
use reth_provider::{BlockIdReader, BlockNumReader, HeaderProvider};
use reth_rpc_api::EthApiServer;
use reth_rpc_types::trace::parity::{CallAction, TraceType};
use reth_tracing::TracingClient;
use tokio::{sync::mpsc::UnboundedSender, task::JoinError};
use tracing::{info, warn};

use crate::{
    errors::TraceParseError,
    executor::{Executor, TaskKind},
    init_trace,
};

fn decode_input_with_abi(
    abi: &JsonAbi,
    action: &CallAction,
    trace_address: &Vec<usize>,
    tx_hash: &H256,
) -> Result<Option<StructuredTrace>, TraceParseError> {
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
                let params_type = DynSolType::Tuple(resolved_params);

                // Remove the function selector from the input.
                let inputs = &action.input[4..];
                // Decode the inputs based on the resolved parameters.
                match params_type.abi_decode(inputs) {
                    Ok(decoded_params) => {
                        info!(
                            "For function {}: Decoded params: {:?} \n, with tx hash: {:#?}",
                            function.name, decoded_params, tx_hash
                        );
                        todo!()
                        // return Ok(Some(StructuredTrace::CALL(CallAction::new(
                        //     action.from,
                        //     action.to,
                        //     function.name.clone(),
                        //     Some(decoded_params),
                        //     trace_address.clone(),
                        // ))))
                    }
                    Err(e) => {
                        warn!(error=?e, "Failed to decode params");
                    }
                }
            }
        }
    }
    Ok(None)
}

fn handle_empty_input(
    abi: &JsonAbi,
    action: &CallAction,
    trace_address: &Vec<usize>,
    tx_hash: &H256,
) -> Result<StructuredTrace, TraceParseError> {
    todo!()
    // if action.value != U256::from(0) {
    //     if let Some(receive) = &abi.receive {
    //         if receive.state_mutability == StateMutability::Payable {
    //             return Ok(StructuredTrace::CALL(CallAction::new(
    //                 action.to,
    //                 action.from,
    //                 RECEIVE.to_string(),
    //                 None,
    //                 trace_address.clone(),
    //             )))
    //         }
    //     }
    //
    //     if let Some(fallback) = &abi.fallback {
    //         if fallback.state_mutability == StateMutability::Payable {
    //             return Ok(StructuredTrace::CALL(CallAction::new(
    //                 action.from,
    //                 action.to,
    //                 FALLBACK.to_string(),
    //                 None,
    //                 trace_address.clone(),
    //             )))
    //         }
    //     }
    // }
    // Err(TraceParseError::EmptyInput(tx_hash.clone().into()))
}
