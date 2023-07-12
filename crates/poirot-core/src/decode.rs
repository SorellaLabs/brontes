use alloy_dyn_abi::DynSolType;
use alloy_json_abi::JsonAbi;
use dotenv::dotenv;
use ethers_core::types::Chain;
use ethers_etherscan::Client;

use ethers::abi::{Abi, Token};
use ethers::types::H160;
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
                Ok(res) => {
                    println!("{res}");
                    result.push(res);
                }
                _ => continue,
            }
        }

        result
    }

    pub async fn parse_trace(&self, trace: &LocalizedTransactionTrace) -> Result<String, ()> {
        let action = match &trace.trace.action {
            RethAction::Call(call) => call,
            _ => return Err(()),
        };

 
        let abi = self.client.contract_abi(action.try_into()).await.unwrap();

        let mut function_selectors = HashMap::new();

        for function in abi.functions() {
            function_selectors.insert(function.short_signature(), function);
        }

        let input_selector = &action.input[..4];

        let function = function_selectors
            .get(input_selector);

        Ok(String::from(function.unwrap().decode_input(&action.input.to_vec()).unwrap()))
    }
}