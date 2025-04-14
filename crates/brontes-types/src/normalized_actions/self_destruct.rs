use std::fmt::Debug;

use alloy_primitives::{Address, U256};
use clickhouse::Row;
use reth_rpc_types::trace::parity::SelfdestructAction;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct SelfdestructWithIndex {
    pub trace_index: u64,
    pub self_destruct: SelfdestructAction,
}

impl SelfdestructWithIndex {
    pub fn new(trace_index: u64, self_destruct: SelfdestructAction) -> Self {
        Self { trace_index, self_destruct }
    }

    pub fn get_address(&self) -> Address {
        self.self_destruct.address
    }

    pub fn get_balance(&self) -> U256 {
        self.self_destruct.balance
    }

    pub fn get_refund_address(&self) -> Address {
        self.self_destruct.refund_address
    }
}
