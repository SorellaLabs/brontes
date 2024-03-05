use std::fmt::Debug;

use clickhouse::Row;
use reth_primitives::{Address, U256};
use serde::{Deserialize, Serialize};

use super::accounting::{AddressDeltas, TokenAccounting};
pub use super::{Actions, NormalizedSwap, NormalizedTransfer};
use crate::{db::token_info::TokenInfoWithAddress, Protocol};

#[derive(Debug, Serialize, Clone, Row, Deserialize, PartialEq, Eq)]
pub struct NormalizedAggregator {
    pub protocol:    Protocol,
    pub trace_index: u64,
    pub from:        Address,
    pub recipient:   Address,

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

impl TokenAccounting for NormalizedAggregator {
    fn apply_token_deltas(&self, delta_map: &mut AddressDeltas) {
        self.child_actions
            .iter()
            .for_each(|action| action.apply_token_deltas(delta_map))
    }
}

impl NormalizedAggregator {
    pub fn finish_classification(&mut self, actions: Vec<(u64, Actions)>) -> Vec<u64> {
        let mut nodes_to_prune = Vec::new();
        let mut token_in_trace_index_counter: u64 = u64::MAX;
        let mut token_in = TokenInfoWithAddress::default();
        if self.protocol == Protocol::OneInchFusion {
            // First, process Swap actions
            for (index, action) in actions
                .iter()
                .filter(|(_, action)| matches!(action, Actions::Swap(_)))
            {
                if let Actions::Swap(swap) = action {
                    token_in = swap.token_in.clone();
                    self.child_actions.push(action.clone());
                    nodes_to_prune.push(*index);
                }
            }

            // Then, process Transfer actions
            for (index, action) in actions
                .iter()
                .filter(|(_, action)| matches!(action, Actions::Transfer(_)))
            {
                if let Actions::Transfer(transfer) = action {
                    if transfer.token == token_in
                        && transfer.trace_index < token_in_trace_index_counter
                    {
                        self.recipient = transfer.from;
                        token_in_trace_index_counter = transfer.trace_index;
                    }
                    self.child_actions.push(action.clone());
                    nodes_to_prune.push(*index);
                }
            }

            // Process other actions (Batch, Burn, Mint)
            for (index, action) in actions.iter().filter(|(_, action)| {
                matches!(action, Actions::Batch(_) | Actions::Burn(_) | Actions::Mint(_))
            }) {
                self.child_actions.push(action.clone());
                nodes_to_prune.push(*index);
            }
        } else {
            for (trace_index, action) in actions {
                match action {
                    Actions::Swap(_)
                    | Actions::Liquidation(_)
                    | Actions::Batch(_)
                    | Actions::Burn(_)
                    | Actions::Mint(_)
                    | Actions::Transfer(_) => {
                        self.child_actions.push(action.clone());
                        nodes_to_prune.push(trace_index);
                    }
                    _ => {}
                }
            }
        }

        nodes_to_prune
    }
}
