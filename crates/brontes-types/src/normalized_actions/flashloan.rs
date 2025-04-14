use std::fmt::Debug;

use clickhouse::Row;
use malachite::Rational;
use alloy_primitives::{Address, U256};
use serde::{Deserialize, Serialize};

use super::accounting::{AddressDeltas, TokenAccounting};
pub use super::{Action, NormalizedSwap, NormalizedTransfer};
use crate::{db::token_info::TokenInfoWithAddress, Protocol};

#[derive(Debug, Serialize, Clone, Row, Deserialize, PartialEq, Eq)]
pub struct NormalizedFlashLoan {
    pub protocol:          Protocol,
    pub trace_index:       u64,
    pub from:              Address,
    pub pool:              Address,
    pub receiver_contract: Address,
    pub assets:            Vec<TokenInfoWithAddress>,
    pub amounts:           Vec<Rational>,
    // Special case for Aave flashloan modes, see:
    // https://docs.aave.com/developers/guides/flash-loans#completing-the-flash-loan
    pub aave_mode:         Option<(Vec<U256>, Address)>,

    // Child actions contained within this flashloan in order of execution
    // They can be:
    //  - Swaps
    //  - Liquidations
    //  - Mints
    //  - Burns
    //  - Transfers
    pub child_actions: Vec<Action>,
    pub repayments:    Vec<NormalizedTransfer>,
    pub fees_paid:     Vec<Rational>,
    pub msg_value:     U256,
}

impl TokenAccounting for NormalizedFlashLoan {
    fn apply_token_deltas(&self, delta_map: &mut AddressDeltas) {
        self.child_actions
            .iter()
            .for_each(|action| action.apply_token_deltas(delta_map))
    }
}

impl NormalizedFlashLoan {
    pub fn fetch_underlying_actions(self) -> impl Iterator<Item = Action> {
        self.child_actions
            .into_iter()
            .chain(self.repayments.into_iter().map(Action::from))
    }
}
