use std::fmt::Debug;

use alloy_primitives::Address;
use serde::{Deserialize, Serialize};
use sorella_db_databases::{clickhouse, clickhouse::Row};

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
}

impl NormalizedBatch {
    pub fn finish_classification(&mut self, _actions: Vec<(u64, Actions)>) -> Vec<u64> {
        todo!("finish classification for batch")
    }
}
