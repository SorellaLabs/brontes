use std::fmt::Debug;


use reth_primitives::{Address, U256};
use serde::{Deserialize, Serialize};
use sorella_db_databases::{clickhouse, clickhouse::Row};

#[derive(Debug, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedTransfer {
    pub trace_index: u64,
    pub from:        Address,
    pub to:          Address,
    pub token:       Address,
    pub amount:      U256,
}
