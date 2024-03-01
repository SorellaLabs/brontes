use std::fmt::Debug;

use clickhouse::Row;
use malachite::Rational;
use reth_primitives::Address;
use serde::{Deserialize, Serialize};

use super::accounting::{apply_delta, AddressDeltas, TokenAccounting};
use crate::db::token_info::TokenInfoWithAddress;

#[derive(Debug, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedTransfer {
    pub trace_index: u64,
    pub from:        Address,
    pub to:          Address,
    pub token:       TokenInfoWithAddress,
    pub amount:      Rational,
    pub fee:         Rational,
}

impl TokenAccounting for NormalizedTransfer {
    fn apply_token_deltas(&self, delta_map: &mut AddressDeltas) {
        let amount_sent = &self.amount + &self.fee;

        apply_delta(self.from, self.token.address, -amount_sent.clone(), delta_map);
        apply_delta(self.to, self.token.address, self.amount.clone(), delta_map);
    }
}
