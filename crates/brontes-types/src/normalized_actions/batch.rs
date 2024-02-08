use std::fmt::Debug;

use alloy_primitives::{Address, U256};
use serde::{Deserialize, Serialize};
use sorella_db_databases::{clickhouse, clickhouse::Row};
use tracing::error;

pub use super::{Actions, NormalizedSwap};
use crate::Protocol;

#[derive(Debug, Default, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedBatch {
    pub protocol:            Protocol,
    pub trace_index:         u64,
    pub solver:              Address,
    pub settlement_contract: Address,
    pub user_swaps:          Vec<NormalizedSwap>,
    pub solver_swaps:        Option<Vec<NormalizedSwap>>,
    pub msg_value:           U256,
}

impl NormalizedBatch {
    pub fn finish_classification(&mut self, actions: Vec<(u64, Actions)>) -> Vec<u64> {
        let mut nodes_to_prune = Vec::new();

        for (trace_index, action) in actions.iter() {
            match &action {
                Actions::Transfer(t) => {
                    for user_swap in &mut self.user_swaps {
                        if t.from == user_swap.from && t.to == self.solver {
                            user_swap.trace_index = *trace_index;
                            user_swap.token_in = t.token.clone();
                            user_swap.amount_in = t.amount.clone();
                            nodes_to_prune.push(*trace_index);
                        } else if t.from == self.solver && t.to == user_swap.from {
                            user_swap.token_out = t.token.clone();
                            user_swap.amount_out = t.amount.clone();
                            nodes_to_prune.push(*trace_index);
                        }
                    }
                }
                Actions::Swap(s) => {
                    if s.from == self.solver {
                        if let Some(swaps) = &mut self.solver_swaps {
                            swaps.push(s.clone());
                            nodes_to_prune.push(*trace_index);
                        } else {
                            self.solver_swaps = Some(vec![s.clone()]);
                            nodes_to_prune.push(*trace_index);
                        }
                    }
                }
                _ => {
                    error!("Unexpected action in final batch classification: {:?}", action);
                }
            }
        }

        nodes_to_prune
    }
}
