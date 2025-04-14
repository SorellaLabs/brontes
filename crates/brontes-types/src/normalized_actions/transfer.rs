use std::fmt::Debug;

use clickhouse::Row;
use malachite::Rational;
use redefined::Redefined;
use alloy_primitives::{Address, U256};
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};

use super::accounting::{apply_delta, AddressDeltas, TokenAccounting};
use crate::{
    db::{
        redefined_types::{malachite::*, primitives::*},
        token_info::{TokenInfoWithAddress, TokenInfoWithAddressRedefined},
    },
    rational_to_u256_fraction,
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
    pub msg_value:   U256,
}

impl TokenAccounting for NormalizedTransfer {
    fn apply_token_deltas(&self, delta_map: &mut AddressDeltas) {
        let amount_sent = &self.amount + &self.fee;

        apply_delta(self.from, self.token.address, -amount_sent.clone(), delta_map);
        apply_delta(self.to, self.token.address, self.amount.clone(), delta_map);
    }
}

pub struct ClickhouseVecNormalizedTransfer {
    pub trace_index: Vec<u64>,
    pub from:        Vec<String>,
    pub to:          Vec<String>,
    pub token:       Vec<(String, String)>,
    pub amount:      Vec<([u8; 32], [u8; 32])>,
    pub fee:         Vec<([u8; 32], [u8; 32])>,
    pub msg_value:   Vec<U256>,
}

impl TryFrom<Vec<NormalizedTransfer>> for ClickhouseVecNormalizedTransfer {
    type Error = eyre::Report;

    fn try_from(value: Vec<NormalizedTransfer>) -> eyre::Result<Self> {
        Ok(ClickhouseVecNormalizedTransfer {
            trace_index: value.iter().map(|val| val.trace_index).collect(),
            from:        value.iter().map(|val| format!("{:?}", val.from)).collect(),
            to:          value.iter().map(|val| format!("{:?}", val.to)).collect(),
            token:       value.iter().map(|val| val.token.clickhouse_fmt()).collect(),
            amount:      value
                .iter()
                .map(|val| rational_to_u256_fraction(&val.amount))
                .collect::<eyre::Result<Vec<_>>>()?,
            fee:         value
                .iter()
                .map(|val| rational_to_u256_fraction(&val.fee))
                .collect::<eyre::Result<Vec<_>>>()?,
            msg_value:   value.iter().map(|val| val.msg_value).collect::<Vec<_>>(),
        })
    }
}
