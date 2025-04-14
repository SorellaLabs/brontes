use alloy_primitives::{Address, U256};
use clickhouse::Row;
use serde::{Deserialize, Serialize};

use super::accounting::{AddressDeltas, TokenAccounting};
pub use super::{Action, NormalizedSwap, NormalizedTransfer};
use crate::Protocol;

#[derive(Debug, Serialize, Clone, Row, Deserialize, PartialEq, Eq)]
pub struct NormalizedAggregator {
    pub protocol: Protocol,
    pub trace_index: u64,
    pub from: Address,
    pub to: Address,
    pub recipient: Address,

    // Child actions contained within this aggregator in order of execution
    // They can be:
    //  - Swaps
    //  - Batchs
    //  - Liquidations
    //  - Mints
    //  - Burns
    //  - Transfers
    pub child_actions: Vec<Action>,
    pub msg_value: U256,
}

impl TokenAccounting for NormalizedAggregator {
    fn apply_token_deltas(&self, delta_map: &mut AddressDeltas) {
        self.child_actions
            .iter()
            .for_each(|action| action.apply_token_deltas(delta_map))
    }
}
