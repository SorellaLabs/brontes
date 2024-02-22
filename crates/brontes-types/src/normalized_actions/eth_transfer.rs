use std::fmt::Debug;

use alloy_primitives::{Address, U256};
use clickhouse::Row;
use serde::{Deserialize, Serialize};

pub use super::{Actions, NormalizedSwap};

#[derive(Debug, Default, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedEthTransfer {
    pub trace_index: u64,
    pub from:        Address,
    pub to:          Address,
    pub value:       U256,
}
