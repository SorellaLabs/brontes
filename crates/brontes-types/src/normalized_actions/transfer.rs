use std::fmt::Debug;

use clickhouse::Row;
use malachite::Rational;
use redefined::Redefined;
use reth_primitives::Address;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};

use super::accounting::{apply_delta, AddressDeltas, TokenAccounting};
use crate::{
    db::{
        redefined_types::{malachite::*, primitives::*},
        token_info::{TokenInfoWithAddress, TokenInfoWithAddressRedefined},
    },
    rational_to_clickhouse_tuple,
};

#[derive(Debug, Default, Serialize, Deserialize, Clone, Row, PartialEq, Eq, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct NormalizedTransfer {
    pub trace_index: u64,
    pub from: Address,
    pub to: Address,
    pub token: TokenInfoWithAddress,
    pub amount: Rational,
    pub fee: Rational,
}

impl TokenAccounting for NormalizedTransfer {
    fn apply_token_deltas(&self, delta_map: &mut AddressDeltas) {
        let amount_sent = &self.amount + &self.fee;

        apply_delta(
            self.from,
            self.token.address,
            -amount_sent.clone(),
            delta_map,
        );
        apply_delta(self.to, self.token.address, self.amount.clone(), delta_map);
    }
}

pub struct ClickhouseVecNormalizedTransfer {
    pub trace_index: Vec<u64>,
    pub from: Vec<String>,
    pub to: Vec<String>,
    pub token: Vec<String>,
    pub amount: Vec<([u8; 32], [u8; 32])>,
    pub fee: Vec<([u8; 32], [u8; 32])>,
}

impl From<Vec<NormalizedTransfer>> for ClickhouseVecNormalizedTransfer {
    fn from(value: Vec<NormalizedTransfer>) -> Self {
        ClickhouseVecNormalizedTransfer {
            trace_index: value.iter().map(|val| val.trace_index).collect(),
            from: value.iter().map(|val| format!("{:?}", val.from)).collect(),
            to: value.iter().map(|val| format!("{:?}", val.to)).collect(),
            token: value.iter().map(|val| format!("{:?}", val.token)).collect(),
            amount: value
                .iter()
                .map(|val| rational_to_clickhouse_tuple(&val.amount))
                .collect(),
            fee: value
                .iter()
                .map(|val| rational_to_clickhouse_tuple(&val.fee))
                .collect(),
        }
    }
}
