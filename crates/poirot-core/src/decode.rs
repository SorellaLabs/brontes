use alloy_dyn_abi::DynSolType;
use alloy_json_abi::JsonAbi;
use dotenv::dotenv;
use ethers_core::types::Chain;
use ethers_etherscan::Client;
use reth_rpc_types::trace::parity::{Action as RethAction, LocalizedTransactionTrace};

use std::{env, time::Duration};

pub fn create_etherscan_client() -> Client {
    dotenv().ok();
    let api_key = env::var("ETHERSCAN_API").expect("ETHERSCAN_API must be set");
    let cache_path = env::current_dir().unwrap().join("src/abicache");
    Client::new_cached(Chain::Mainnet, api_key, Some(cache_path), Duration::from_secs(60)).unwrap()
}

// TODO: need to add handling for delegate calls where we fetch the implementation abi
// TODO: this can easily be done by checking the call action type in the trace & using the api

pub async fn sleuth(
    client: &Client,
    trace: LocalizedTransactionTrace,
) -> Result<String, Box<dyn std::error::Error>> {
    let action = trace.trace.action;

    let (contract_address, input) = match action {
        RethAction::Call(call_action) => (call_action.to, call_action.input.to_vec()),
        _ => return Err(From::from("The action in the transaction trace is not Call(CallAction)")),
    };

    //TODO: add checks for contract call types so if delegate call then fetch implementation abi
    //TODO: Also check this code, because i have mostly been working on organising & research not
    // so much on the code
    let metadata = client.contract_source_code(contract_address.into()).await?;

    let abi_str = &metadata.items[0].abi;
    let json_abi: JsonAbi = serde_json::from_str(abi_str)?;

    let function_selector = &input[..4];

    if let Some(functions) = Some(json_abi.functions.values().flatten()) {
        for function in functions {
            if function.selector() == function_selector {
                let input_types: Vec<String> =
                    function.inputs.iter().map(|input| input.to_string()).collect();

                let mut decoded_inputs = Vec::new();
                for (index, input_type_str) in input_types.iter().enumerate() {
                    let input_data = &input[4 + index..]; // Skip the function selector and previous inputs
                    let dyn_sol_type: DynSolType = input_type_str.parse().unwrap();
                    let dyn_sol_value = dyn_sol_type.decode_params(input_data)?;
                    decoded_inputs.push(format!("{:?}", dyn_sol_value));
                }

                let printout = format!("Function: {}\nInputs: {:?}", function.name, decoded_inputs);
                return Ok(printout)
            }
        }
    }

    Err(From::from("No matching function found in the ABI"))
}
