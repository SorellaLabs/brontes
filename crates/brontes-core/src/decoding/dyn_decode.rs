use std::{collections::HashSet, path::PathBuf, pin::Pin, sync::Arc};

use alloy_dyn_abi::*;
use alloy_etherscan::Client;
use alloy_json_abi::JsonAbi;
use brontes_types::structured_trace::{DecodedCallData, DecodedParams, TxTrace};
use ethers::prelude::{Http, Middleware, Provider};
use ethers_core::types::Chain;
use ethers_reth::type_conversions::{ToEthers, ToReth};
use futures::Future;
use reth_interfaces::RethError;
use reth_primitives::{BlockId, BlockNumber, BlockNumberOrTag, Header, H256};
use reth_provider::{BlockIdReader, BlockNumReader, HeaderProvider};
use reth_rpc_api::EthApiServer;
use reth_rpc_types::trace::parity::{Action, CallAction, TraceOutput, TraceType, TransactionTrace};
use reth_tracing::TracingClient;
use tokio::{sync::mpsc::UnboundedSender, task::JoinError};
use tracing::{info, warn};

use crate::errors::TraceParseError;

const FALLBACK: &str = "fallback";
const RECEIVE: &str = "receive";

pub fn decode_input_with_abi(
    abi: &JsonAbi,
    trace: &TransactionTrace,
) -> Result<Option<DecodedCallData>, TraceParseError> {
    let Action::Call(action) = trace.action else { return Ok(None) };

    for functions in abi.functions.values() {
        for function in functions {
            if function.selector() == action.input[..4] {
                // Resolve all inputs
                let mut resolved_params: Vec<DynSolType> = function
                    .inputs
                    .iter()
                    .filter_map(|param| param.resolve().ok())
                    .collect();

                let mut input_names = function.inputs.iter().map(|f| f.name).collect::<Vec<_>>();
                let input_params_type = DynSolType::Tuple(resolved_params);

                let mut resolved_output_params: Vec<DynSolType> = function
                    .outputs
                    .iter()
                    .filter_map(|param| param.resolve().ok())
                    .collect();

                let mut output_names = function.outputs.iter().map(|f| f.name).collect::<Vec<_>>();
                let output_type = DynSolType::Tuple(resolved_output_params);

                // Remove the function selector from the input.
                let inputs = &action.input[4..];
                let mut input_results = Vec::new();

                // decode input
                decode_params(
                    input_params_type.abi_decode(inputs)?,
                    &mut input_names,
                    &mut input_results,
                );

                // decode output if exists
                let output = if let Some(TraceOutput::Call(output)) = trace.result {
                    let mut output_results = Vec::new();
                    decode_params(
                        output_type.abi_decode(output.output),
                        &mut output_names,
                        &mut output_results,
                    )
                } else {
                    vec![]
                };

                Ok(DecodedCallData {
                    function_name: function.name,
                    call_data:     input_results,
                    return_data:   output,
                })
            }
        }
    }
    Ok(None)
}

fn handle_empty_input(
    abi: &JsonAbi,
    action: &CallAction,
) -> Result<DecodedCallData, TraceParseError> {
    todo!()
}

fn decode_params(
    sol_value: DynSolValue,
    field_name: &mut Vec<String>,
    output: &mut Vec<DecodedParams>,
) {
    match sol_value {
        /// A boolean.
        DynSolValue::Bool(bool) => output.push(DecodedParams {
            field_name: field_name.remove(0),
            field_type: DynSolType::Bool.sol_type_name().to_string(),
            value:      bool.to_string(),
        }),
        /// A signed integer.
        DynSolValue::Int(i, size) => output.push(DecodedParams {
            field_name: field_name.remove(0),
            field_type: DynSolType::Int(size).to_string(),
            value:      i.to_string(),
        }),
        /// An unsigned integer.
        DynSolValue::Uint(i, size) => output.push(DecodedParams {
            field_name: field_name.remove(0),
            field_type: DynSolType::Uint(size).to_string(),
            value:      i.to_string(),
        }),
        /// A fixed-length byte string.
        DynSolValue::FixedBytes(word, size) => output.push(DecodedParams {
            field_name: field_name.remove(0),
            field_type: DynSolType::FixedBytes(size).to_string(),
            value:      word.to_string(),
        }),
        /// An address.
        DynSolValue::Address(address) => output.push(DecodedParams {
            field_name: field_name.remove(0),
            field_type: DynSolType::Address.to_string(),
            value:      format!("{:?}", address),
        }),
        /// A function pointer.
        DynSolValue::Function(function) => output.push(DecodedParams {
            field_name: field_name.remove(0),
            field_type: DynSolType::Function.to_string(),
            value:      function.to_string(),
        }),

        /// A dynamic-length byte array.
        DynSolValue::Bytes(bytes) => output.push(DecodedParams {
            field_name: field_name.remove(0),
            field_type: DynSolType::Bytes.to_string(),
            value:      alloy_primitives::Bytes::from(bytes).to_string(),
        }),
        /// A string.
        DynSolValue::String(string) => output.push(DecodedParams {
            field_name: field_name.remove(0),
            field_type: DynSolType::String.to_string(),
            value:      string,
        }),

        /// A dynamically-sized array of values.
        DynSolValue::Array(array) => {
            let string_val = value_parse(array, false);
            let type_name = sol_value.sol_type_name().unwrap().to_string();
            output.push(DecodedParams {
                field_name: field_name.remove(0),
                field_type: type_name,
                value:      string_val,
            })
        }
        /// A fixed-size array of values.
        DynSolValue::FixedArray(fixed_array) => {
            let string_val = value_parse(fixed_array, false);
            let type_name = sol_value.sol_type_name().unwrap().to_string();
            output.push(DecodedParams {
                field_name: field_name.remove(0),
                field_type: type_name,
                value:      string_val,
            })
        }
        /// A tuple of values.
        DynSolValue::Tuple(tuple) => {
            let string_val = value_parse(tuple, true);
            let type_name = sol_value.sol_type_name().unwrap().to_string();
            output.push(DecodedParams {
                field_name: field_name.remove(0),
                field_type: type_name,
                value:      string_val,
            })
        }
    }
}

fn value_parse(sol_value: Vec<DynSolValue>, tuple: bool) -> String {
    let ty = if tuple { String::from("(") } else { String::from("[") };

    let unclosed = sol_value
        .into_iter()
        .map(|t| match t {
            DynSolValue::Bool(bool) => bool.to_string(),
            DynSolValue::Int(i, _) => i.to_string(),
            DynSolValue::Uint(i, _) => i.to_string(),
            DynSolValue::FixedBytes(i, _) => i.to_string(),
            DynSolValue::Address(a) => format!("{:?}", a),
            DynSolValue::Function(f) => f.to_string(),
            DynSolValue::String(s) => s,
            DynSolValue::Bytes(b) => alloy_primitives::Bytes::from(b).to_string(),
            DynSolValue::Tuple(t) => value_parse(t, true),
            DynSolValue::Array(a) => value_parse(a, false),
            DynSolValue::FixedArray(a) => value_parse(a, false),
        })
        .fold(ty, |a, b| a + "," + &b);

    if tuple {
        unclosed + ")"
    } else {
        unclosed + "]"
    }
}
