use std::fmt::Debug;

use alloy_primitives::{Address, U256};
use serde::{Deserialize, Serialize};
use sorella_db_databases::{clickhouse, clickhouse::Row};

pub use super::{Actions, NormalizedSwap};

#[derive(Debug, Default, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedEthTransfer {
    pub trace_index: u64,
    pub from: Address,
    pub to: Address,
    pub value: U256,
}
