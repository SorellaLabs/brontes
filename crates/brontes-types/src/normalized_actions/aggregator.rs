use std::fmt::Debug;

use clickhouse::Row;
use malachite::Rational;
use reth_primitives::{Address, U256};
use serde::{Deserialize, Serialize};

pub use super::{Actions, NormalizedSwap, NormalizedTransfer};
use crate::{db::token_info::TokenInfoWithAddress, Protocol};

#[derive(Debug, Serialize, Clone, Row, Deserialize, PartialEq, Eq)]
pub struct NormalizedAggregator {
    pub protocol:    Protocol,
    pub trace_index: u64,
    pub from:        Address,
    pub recipient:   Address,
    pub pool:        Address,
    pub token_in:    TokenInfoWithAddress,
    pub token_out:   TokenInfoWithAddress,
    pub amount_in:   Rational,
    pub amount_out:  Rational,

    // Child actions contained within this aggregator in order of execution
    // They can be:
    //  - Swaps
    //  - Batchs
    //  - Liquidations
    //  - Mints
    //  - Burns
    //  - Transfers
    pub child_actions: Vec<Actions>,
    pub msg_value:     U256,
}

impl NormalizedAggregator {
    pub fn finish_classification(&mut self, actions: Vec<(u64, Actions)>) -> Vec<u64> {
        let mut nodes_to_prune = Vec::new();
        for (trace_index, action) in actions.iter() {
            match &action {
                Actions::Swap(_)
                | Actions::Liquidation(_)
                | Actions::Batch(_)
                | Actions::Burn(_)
                | Actions::Mint(_)
                | Actions::Transfer(_) => {
                    self.child_actions.push(action.clone());
                    nodes_to_prune.push(*trace_index);
                }
                _ => continue,
            }
        }
        nodes_to_prune
    }
}
