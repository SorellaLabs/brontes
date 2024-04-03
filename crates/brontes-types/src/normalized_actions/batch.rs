use std::fmt::Debug;

use alloy_primitives::{Address, U256};
use clickhouse::Row;
use serde::{Deserialize, Serialize};
use tracing::error;

use super::accounting::{apply_delta, AddressDeltas, TokenAccounting};
pub use super::{Actions, NormalizedSwap};
use crate::{db::token_info::TokenInfoWithAddress, utils::ToScaledRational, Protocol};

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
                            break
                        } else if t.from == self.solver && t.to == user_swap.from {
                            user_swap.token_out = t.token.clone();
                            user_swap.amount_out = t.amount.clone();
                            break
                        }
                    }
                }
                Actions::EthTransfer(et) => {
                    for user_swap in &mut self.user_swaps {
                        if et.from == user_swap.from && et.to == self.settlement_contract {
                            user_swap.trace_index = *trace_index;
                            user_swap.token_in = TokenInfoWithAddress::native_eth();
                            user_swap.amount_in = et.clone().value.to_scaled_rational(18);
                            break
                        } else if et.from == self.settlement_contract && et.to == user_swap.from {
                            user_swap.token_out = TokenInfoWithAddress::native_eth();
                            user_swap.amount_out = et.clone().value.to_scaled_rational(18);
                            break
                        }
                    }
                }
                Actions::Swap(s) => {
                    if let Some(swaps) = &mut self.solver_swaps {
                        swaps.push(s.clone());
                        nodes_to_prune.push(*trace_index);
                        break
                    } else {
                        self.solver_swaps = Some(vec![s.clone()]);
                        nodes_to_prune.push(*trace_index);
                        break
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

impl TokenAccounting for NormalizedBatch {
    fn apply_token_deltas(&self, delta_map: &mut AddressDeltas) {
        self.user_swaps.iter().for_each(|swap| {
            apply_delta(self.solver, swap.token_in.address, swap.amount_in.clone(), delta_map);
            apply_delta(self.solver, swap.token_out.address, -swap.amount_out.clone(), delta_map);

            swap.apply_token_deltas(delta_map);
        });

        if let Some(swaps) = &self.solver_swaps {
            swaps
                .iter()
                .for_each(|swap| swap.apply_token_deltas(delta_map));
        }
    }
}
