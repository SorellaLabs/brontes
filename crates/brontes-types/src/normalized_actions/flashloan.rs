use std::fmt::Debug;

use clickhouse::Row;
use malachite::Rational;
use reth_primitives::{Address, U256};
use serde::{Deserialize, Serialize};

use super::accounting::{AddressDeltas, TokenAccounting};
pub use super::{Actions, NormalizedSwap, NormalizedTransfer};
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
    pub child_actions: Vec<Actions>,
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
    pub fn fetch_underlying_actions(self) -> impl Iterator<Item = Actions> {
        self.child_actions
            .into_iter()
            .chain(self.repayments.into_iter().map(Actions::from))
    }

    pub fn finish_classification(&mut self, actions: Vec<(u64, Actions)>) -> Vec<u64> {
        let mut nodes_to_prune = Vec::new();
        let mut a_token_addresses = Vec::new();
        let mut repay_transfers = Vec::new();

        for (index, action) in actions.into_iter() {
            match &action {
                Actions::Swap(_)
                | Actions::FlashLoan(_)
                | Actions::Liquidation(_)
                | Actions::Batch(_)
                | Actions::Burn(_)
                | Actions::EthTransfer(_)
                | Actions::Mint(_) => {
                    self.child_actions.push(action);
                    nodes_to_prune.push(index);
                }
                Actions::Transfer(t) => {
                    // get the a_token reserve address that will be the receiver of the flashloan
                    // repayment for this token
                    if let Some(i) = self.assets.iter().position(|x| *x == t.token) {
                        if t.to == self.receiver_contract && t.amount == self.amounts[i] {
                            a_token_addresses.push(t.token.address);
                        }
                    }
                    // if the receiver contract is sending the token to the AToken address then this
                    // is the flashloan repayment
                    else if t.from == self.receiver_contract && a_token_addresses.contains(&t.to)
                    {
                        repay_transfers.push(t.clone());
                        nodes_to_prune.push(index);
                        continue
                    // replayment back to the flash-loan pool
                    } else if t.from == self.receiver_contract && self.pool == t.to {
                        if let Some(i) = self.assets.iter().position(|x| *x == t.token) {
                            if t.amount >= self.amounts[i] {
                                repay_transfers.push(t.clone());
                                nodes_to_prune.push(index);
                                continue
                            }
                        }
                    }
                    self.child_actions.push(action);
                    nodes_to_prune.push(index);
                }
                _ => continue,
            }
        }
        let fees = Vec::new();

        // //TODO: deal with diff aave modes, where part of the flashloan is taken on as
        // // debt by the OnBehalfOf address
        // for (i, amount) in self.amounts.iter().enumerate() {
        //     let repay_amount = repay_transfers
        //         .iter()
        //         .find(|t| t.token == self.assets[i])
        //         .map_or(U256::ZERO, |t| t.amount);
        //     let fee = repay_amount - amount;
        //     fees.push(fee);
        // }

        self.fees_paid = fees;
        self.repayments = repay_transfers;

        nodes_to_prune
    }
}
