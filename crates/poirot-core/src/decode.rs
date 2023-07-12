use alloy_dyn_abi::DynSolType;
use alloy_json_abi::JsonAbi;
use dotenv::dotenv;
use ethers_core::types::Chain;
use ethers_etherscan::Client;
use reth_rpc_types::trace::parity::{Action as RethAction, LocalizedTransactionTrace};

use std::{env, time::Duration};

pub struct Parser {
    block_trace: Vec<LocalizedTransactionTrace>,
    client: Client,
}

impl Parser {
    pub fn new(block_trace: Vec<LocalizedTransactionTrace>, etherscan_key: String) -> Self {
        Self {
            block_trace,
            client: Client::new(Chain::Mainnet, etherscan_key).unwrap(),
        }
    }

    pub async fn parse(&self) -> Vec<String> {
        let mut result = vec![];

        for trace in &self.block_trace {
            match self.parse_trace(trace).await {
                Ok(res) => result.push(res),
                _ => continue,
            }
        }

        result
    }

    pub async fn parse_trace(&self, trace: &LocalizedTransactionTrace) -> Result<String, Box<dyn std::error::Error>> {
        let action = &trace.trace.action;

        let (contract_address, input) = match action {
            RethAction::Call(call_action) => (call_action.to, call_action.input.to_vec()),
            _ => return Err(From::from("The action in the transaction trace is not Call(CallAction)")),
        };
    
        let metadata = self.client.contract_source_code(contract_address.into()).await?;
    
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

                        let ty = input_type_str.split_whitespace().next().unwrap();

                        let dyn_sol_type: DynSolType = ty.parse().unwrap();
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
}