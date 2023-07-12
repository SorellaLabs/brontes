use alloy_dyn_abi::DynSolType;
use alloy_json_abi::JsonAbi;
use dotenv::dotenv;
use ethers_core::types::Chain;
use ethers_etherscan::Client;

use ethers::{
    abi::{Abi, Token},
    types::H160,
};
use reth_rpc_types::trace::parity::{Action as RethAction, LocalizedTransactionTrace};

use std::{env, time::Duration};

/// A [`Parser`] will iterate through a block's Parity traces and attempt to decode each call for
/// later analysis.
pub struct Parser {
    /// Parity block traces.
    pub block_trace: Vec<LocalizedTransactionTrace>,
    /// Etherscan client for fetching ABI for each contract address.
    pub client: Client,
}

impl Parser {
    /// Public constructor function to instantiate a new [`Parser`].
    /// # Arguments
    /// * `block_trace` - Block trace from [`TracingClient`].
    /// * `etherscan_key` - Etherscan API key to instantiate client.
    pub fn new(block_trace: Vec<LocalizedTransactionTrace>, etherscan_key: String) -> Self {
        Self { block_trace, client: Client::new(Chain::Mainnet, etherscan_key).unwrap() }
    }

    /// Attempt to parse each trace in a block.
    pub async fn parse(&self) -> Vec<Vec<Token>> {
        let mut result = vec![];

        for trace in &self.block_trace {
            match self.parse_trace(trace).await {
                Ok(res) => {
                    println!("{res:#?}");
                    result.push(res);
                }
                _ => continue,
            }
        }

        result
    }

    /// Parse an individual block trace.
    /// # Arguments
    /// * `trace` - Individual block trace.
    pub async fn parse_trace(&self, trace: &LocalizedTransactionTrace) -> Result<Vec<Token>, ()> {
        let action = match &trace.trace.action {
            RethAction::Call(call) => call,
            _ => return Err(()),
        };


        let abi = match self.client.contract_abi(H160(action.to.to_fixed_bytes())).await {
            Ok(abi) => abi,
            Err(_) => return Err(()),
        };

        let mut function_selectors = std::collections::HashMap::new();

        for function in abi.functions() {
            function_selectors.insert(function.short_signature(), function);
        }

        let input_selector = &action.input[..4];

        let function = function_selectors.get(input_selector);

        Ok(function.unwrap().decode_input(&(&action.input.to_vec())[4..]).unwrap())
    }
}
