use std::fmt::Debug;

use clickhouse::Row;
use malachite::Rational;
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
        let mut token_out_trace_index_counter: u64 = 0;
        let mut token_in_trace_index_counter: u64 = 0;
        if self.protocol == Protocol::OneInchFusion {
            for (index, action) in actions {
                match &action {
                    Actions::Swap(swap) => {
                        self.amount_in = swap.amount_in.clone();
                        self.amount_out = swap.amount_out.clone();
                        self.token_in = swap.token_in.clone();
                        self.token_out = swap.token_out.clone();
                        self.child_actions.push(action.clone());
                        nodes_to_prune.push(index);
                    }
                    Actions::Transfer(transfer) => {
                        if transfer.token == self.token_out
                            && transfer.trace_index > token_out_trace_index_counter
                        {
                            self.amount_out = transfer.amount.clone();
                            token_out_trace_index_counter = transfer.trace_index;
                        }
                        if transfer.token == self.token_in
                            && transfer.trace_index < token_in_trace_index_counter
                        {
                            self.recipient = transfer.from;
                            token_in_trace_index_counter = transfer.trace_index;
                        }
                        self.child_actions.push(action.clone());
                        nodes_to_prune.push(index);
                    }
                    Actions::Batch(_) | Actions::Burn(_) | Actions::Mint(_) => {
                        self.child_actions.push(action.clone());
                        nodes_to_prune.push(index);
                    }
                    _ => {}
                }
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
