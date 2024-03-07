use std::{cmp::Ordering, fmt::Debug};

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

    // Child actions contained within this aggregator in this order
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
            let mut lowest_swap_index = None;
            for (index, action) in actions
                .iter()
                .enumerate()
                .filter(|(_, action)| matches!(action.1, Actions::Swap(_)))
            {
                if let Actions::Swap(swap) = &action.1 {
                    if lowest_swap_index.map_or(true, |idx| index < idx) {
                        token_in = swap.token_in.clone();
                        lowest_swap_index = Some(index);
                    }
                    self.child_actions.push(action.1.clone());
                    nodes_to_prune.push(index.try_into().unwrap());
                }
            }

            // Then, process Transfer actions
            for (index, action) in actions.iter().enumerate().filter(|(_, action)| {
                matches!(
                    action.1,
                    Actions::Transfer(_) | Actions::Batch(_) | Actions::Burn(_) | Actions::Mint(_)
                )
            }) {
                if let Actions::Transfer(transfer) = &action.1 {
                    if transfer.token == token_in
                        && transfer.trace_index < token_in_trace_index_counter
                    {
                        self.recipient = transfer.from;
                        token_in_trace_index_counter = transfer.trace_index;
                    }
                }

                self.child_actions.push(action.1.clone());
                nodes_to_prune.push(index.try_into().unwrap());
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
        self.sort_child_actions();
        nodes_to_prune
    }

    fn sort_child_actions(&mut self) {
        self.child_actions.sort_by(|a, b| {
            use Actions::*;
            match (a, b) {
                (Swap(_), _) => Ordering::Less,
                (_, Swap(_)) => Ordering::Greater,
                (Batch(_), _) => Ordering::Less,
                (_, Batch(_)) => Ordering::Greater,
                (Liquidation(_), _) => Ordering::Less,
                (_, Liquidation(_)) => Ordering::Greater,
                (Mint(_), _) => Ordering::Less,
                (_, Mint(_)) => Ordering::Greater,
                (Burn(_), _) => Ordering::Less,
                (_, Burn(_)) => Ordering::Greater,
                (Transfer(_), Transfer(_)) => Ordering::Equal,
                (Transfer(_), _) => Ordering::Less,
                (_, Transfer(_)) => Ordering::Greater,
                _ => Ordering::Equal, // Ignore any other variants
            }
        });
    }
}
