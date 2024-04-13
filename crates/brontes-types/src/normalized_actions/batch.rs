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
    pub fn fetch_underlying_actions(self) -> impl Iterator<Item = Actions> {
        self.user_swaps
            .into_iter()
            .chain(self.solver_swaps.unwrap_or_default())
            .map(Actions::from)
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
