use std::fmt::Debug;

use clickhouse::Row;
use malachite::Rational;
use redefined::Redefined;
use reth_primitives::Address;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};

use super::accounting::{apply_delta, AddressDeltas, TokenAccounting};
use crate::db::{
    redefined_types::{malachite::*, primitives::*},
    token_info::{TokenInfoWithAddress, TokenInfoWithAddressRedefined},
};

#[derive(Debug, Default, Serialize, Deserialize, Clone, Row, PartialEq, Eq, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
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
