use alloy_dyn_abi::*;
use alloy_json_abi::JsonAbi;
use alloy_rpc_types::trace::parity::{Action, TraceOutput, TransactionTrace};
use brontes_types::structured_trace::{DecodedCallData, DecodedParams};

use crate::errors::TraceParseError;

pub fn decode_input_with_abi(
    abi: &JsonAbi,
    trace: &TransactionTrace,
) -> Result<Option<DecodedCallData>, TraceParseError> {
    let Action::Call(ref action) = trace.action else {
        return Ok(None);
    };

    for functions in abi.functions.values() {
        for function in functions {
            if function.selector() == action.input[..4] {
                // Resolve all inputs
                let resolved_params: Vec<DynSolType> = function
                    .inputs
                    .iter()
                    .filter_map(|param| param.resolve().ok())
                    .collect();

                let mut input_names = function
                    .inputs
                    .iter()
                    .map(|f| f.name.clone())
                    .collect::<Vec<_>>();
                let input_params_type = DynSolType::Tuple(resolved_params);

                let resolved_output_params: Vec<DynSolType> = function
                    .outputs
                    .iter()
                    .filter_map(|param| param.resolve().ok())
                    .collect();

                let mut output_names = function
                    .outputs
                    .iter()
                    .map(|f| f.name.clone())
                    .collect::<Vec<_>>();
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
                let output = if let Some(TraceOutput::Call(output)) = &trace.result {
                    let mut output_results = Vec::new();
                    decode_params(
                        output_type.abi_decode(&output.output)?,
                        &mut output_names,
                        &mut output_results,
                    );
                    output_results
                } else {
                    vec![]
                };

                return Ok(Some(DecodedCallData {
                    function_name: function.name.clone(),
                    call_data: input_results,
                    return_data: output,
                }));
            }
        }
    }
    Ok(None)
}

fn decode_params(
    sol_value: DynSolValue,
    field_name: &mut Vec<String>,
    output: &mut Vec<DecodedParams>,
) {
    match sol_value {
        DynSolValue::Bool(bool) => output.push(DecodedParams {
            field_name: field_name.remove(0),
            field_type: DynSolType::Bool.sol_type_name().to_string(),
            value: bool.to_string(),
        }),
        DynSolValue::Int(i, size) => output.push(DecodedParams {
            field_name: field_name.remove(0),
            field_type: DynSolType::Int(size).to_string(),
            value: i.to_string(),
        }),
        DynSolValue::Uint(i, size) => output.push(DecodedParams {
            field_name: field_name.remove(0),
            field_type: DynSolType::Uint(size).to_string(),
            value: i.to_string(),
        }),
        DynSolValue::FixedBytes(word, size) => output.push(DecodedParams {
            field_name: field_name.remove(0),
            field_type: DynSolType::FixedBytes(size).to_string(),
            value: word.to_string(),
        }),
        DynSolValue::Address(address) => output.push(DecodedParams {
            field_name: field_name.remove(0),
            field_type: DynSolType::Address.to_string(),
            value: format!("{:?}", address),
        }),
        DynSolValue::Function(function) => output.push(DecodedParams {
            field_name: field_name.remove(0),
            field_type: DynSolType::Function.to_string(),
            value: function.to_string(),
        }),
        DynSolValue::Bytes(bytes) => output.push(DecodedParams {
            field_name: field_name.remove(0),
            field_type: DynSolType::Bytes.to_string(),
            value: alloy_primitives::Bytes::from(bytes).to_string(),
        }),
        DynSolValue::String(string) => output.push(DecodedParams {
            field_name: field_name.remove(0),
            field_type: DynSolType::String.to_string(),
            value: string,
        }),
        DynSolValue::Array(ref array) => {
            let string_val = value_parse(array, false);
            let type_name = sol_value.sol_type_name().unwrap().to_string();
            output.push(DecodedParams {
                field_name: field_name.remove(0),
                field_type: type_name,
                value: string_val,
            })
        }
        DynSolValue::FixedArray(ref fixed_array) => {
            let string_val = value_parse(fixed_array, false);
            let type_name = sol_value.sol_type_name().unwrap().to_string();
            output.push(DecodedParams {
                field_name: field_name.remove(0),
                field_type: type_name,
                value: string_val,
            })
        }
        DynSolValue::Tuple(ref tuple) => {
            let string_val = value_parse(tuple, true);
            let type_name = sol_value.sol_type_name().unwrap().to_string();
            output.push(DecodedParams {
                field_name: field_name.remove(0),
                field_type: type_name,
                value: string_val,
            })
        }
    }
}

fn value_parse(sol_value: &[DynSolValue], tuple: bool) -> String {
    let ty = if tuple { String::from("(") } else { String::from("[") };

    let unclosed = sol_value
        .iter()
        .map(|t| match t {
            DynSolValue::Bool(bool) => bool.to_string(),
            DynSolValue::Int(i, _) => i.to_string(),
            DynSolValue::Uint(i, _) => i.to_string(),
            DynSolValue::FixedBytes(i, _) => i.to_string(),
            DynSolValue::Address(a) => format!("{:?}", a),
            DynSolValue::Function(f) => f.to_string(),
            DynSolValue::String(s) => s.to_string(),
            DynSolValue::Bytes(b) => alloy_primitives::Bytes::from(b.clone()).to_string(),
            DynSolValue::Tuple(t) => value_parse(t, true),
            DynSolValue::Array(a) => value_parse(a, false),
            DynSolValue::FixedArray(a) => value_parse(a, false),
        })
        .fold(ty, |a, b| a + "," + b.as_str());

    if tuple {
        unclosed + ")"
    } else {
        unclosed + "]"
    }
}
