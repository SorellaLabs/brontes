use std::fmt::Debug;

use alloy_primitives::{Address, U256};
use clickhouse::Row;
use serde::{Deserialize, Serialize};

use super::accounting::{apply_delta, AddressDeltas, TokenAccounting};
pub use super::{Actions, NormalizedSwap};
use crate::{constants::ETH_ADDRESS, ToScaledRational};

#[derive(Debug, Default, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedEthTransfer {
    pub trace_index:       u64,
    pub from:              Address,
    pub to:                Address,
    pub value:             U256,
    pub coinbase_transfer: bool,
}

impl TokenAccounting for NormalizedEthTransfer {
    fn apply_token_deltas(&self, delta_map: &mut AddressDeltas) {
        // Do not account for coinbase transfers as they are accounted in the gas cost
        // calculation
        if self.coinbase_transfer {
            return;
        }

        let am = self.value.to_scaled_rational(18);

        apply_delta(self.from, ETH_ADDRESS, -am.clone(), delta_map);
        apply_delta(self.to, ETH_ADDRESS, am, delta_map);
    }
}
