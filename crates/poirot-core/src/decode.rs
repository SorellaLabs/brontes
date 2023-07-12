use ethers_core::types::Chain;
use ethers_etherscan::Client;
use ethers::abi::ParamType;

use crate::action::Action;
use ethers::types::H160;
use reth_rpc_types::trace::parity::{Action as RethAction, LocalizedTransactionTrace};
use std::path::PathBuf;

use alloy_dyn_abi::{DynSolType, DynSolValue};

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
        Self {
            block_trace,
            client: Client::new_cached(
                Chain::Mainnet,
                etherscan_key,
                Some(PathBuf::from("./abi_cache")),
                std::time::Duration::new(5, 0),
            )
            .unwrap(),
        }
    }

    /// Attempt to parse each trace in a block.
    pub async fn parse(&self) -> Vec<Action> {
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
    pub async fn parse_trace(&self, trace: &LocalizedTransactionTrace) -> Result<Action, ()> {
        // We only care about "Call" traces, so we extract them here.
        let action = match &trace.trace.action {
            RethAction::Call(call) => call,
            _ => return Err(()),
        };

        // We cannot decode a call for which calldata is zero.
        // TODO: Parse this as a fallback function.
        if action.input.len() <= 0 {
            return Err(())
        }

        // Attempt to fetch the contract ABI from etherscan.
        let abi = match self.client.contract_abi(H160(action.to.to_fixed_bytes())).await {
            Ok(abi) => abi,
            Err(_) => return Err(()),
        };

        for function in abi.functions() {
            if function.short_signature() == &action.input[..4] {
                /*let input_types: Vec<ParamType> = function
                    .inputs
                    .iter()
                    .map(|input| input.kind.clone())
                    .collect();
                */

                // Print input_types
                println!("Input types for funnction {:?}: {:?}", function.name, function.inputs);
                
            }
            
        }
        Err(())
    }
}
