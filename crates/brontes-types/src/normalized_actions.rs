use std::{collections::HashMap, fmt::Debug};

use alloy_primitives::Log;
use reth_primitives::{Address, U256};
use serde::{Deserialize, Serialize};
use sorella_db_databases::clickhouse::{self, InsertRow, Row};

use crate::structured_trace::TransactionTraceWithLogs;

/// A normalized action that has been classified
#[derive(Debug, Clone, Deserialize)]
pub enum Actions {
    Swap(NormalizedSwap),
    FlashLoan(NormalizedFlashLoan),
    Batch(NormalizedBatch),
    Transfer(NormalizedTransfer),
    Mint(NormalizedMint),
    Burn(NormalizedBurn),
    Collect(NormalizedCollect),
    Liquidation(NormalizedLiquidation),
    Unclassified(TransactionTraceWithLogs),
    Revert,
}

impl InsertRow for Actions {
    fn get_column_names(&self) -> &'static [&'static str] {
        match self {
            Actions::Swap(_) => NormalizedSwap::COLUMN_NAMES,
            Actions::FlashLoan(_) => NormalizedFlashLoan::COLUMN_NAMES,
            Actions::Batch(_) => NormalizedBatch::COLUMN_NAMES,
            Actions::Transfer(_) => NormalizedTransfer::COLUMN_NAMES,
            Actions::Mint(_) => NormalizedMint::COLUMN_NAMES,
            Actions::Burn(_) => NormalizedBurn::COLUMN_NAMES,
            Actions::Collect(_) => NormalizedCollect::COLUMN_NAMES,
            Actions::Liquidation(_) => NormalizedLiquidation::COLUMN_NAMES,
            Actions::Unclassified(..) | Actions::Revert => panic!(),
        }
    }
}

impl Serialize for Actions {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Actions::Swap(s) => s.serialize(serializer),
            Actions::FlashLoan(f) => f.serialize(serializer),
            Actions::Batch(b) => b.serialize(serializer),
            Actions::Mint(m) => m.serialize(serializer),
            Actions::Transfer(t) => t.serialize(serializer),
            Actions::Burn(b) => b.serialize(serializer),
            Actions::Collect(c) => c.serialize(serializer),
            Actions::Liquidation(c) => c.serialize(serializer),
            Actions::Unclassified(trace) => (trace).serialize(serializer),
            _ => unreachable!(),
        }
    }
}

impl Actions {
    pub fn force_swap(self) -> NormalizedSwap {
        match self {
            Actions::Swap(s) => s,
            _ => unreachable!(),
        }
    }

    pub fn force_swap_ref(&self) -> &NormalizedSwap {
        match self {
            Actions::Swap(s) => s,
            _ => unreachable!(),
        }
    }

    pub fn get_logs(&self) -> Vec<Log> {
        match self {
            Self::Unclassified(a) => a.logs.clone(),
            _ => vec![],
        }
    }

    pub fn get_to_address(&self) -> Address {
        match self {
            Actions::Swap(s) => s.pool,
            Actions::FlashLoan(f) => f.pool,
            Actions::Batch(b) => b.settlement_contract,
            Actions::Mint(m) => m.to,
            Actions::Burn(b) => b.to,
            Actions::Transfer(t) => t.to,
            Actions::Collect(c) => c.to,
            Actions::Liquidation(c) => c.pool,
            Actions::Unclassified(t) => match &t.trace.action {
                reth_rpc_types::trace::parity::Action::Call(c) => c.to,
                reth_rpc_types::trace::parity::Action::Create(_) => Address::ZERO,
                reth_rpc_types::trace::parity::Action::Reward(_) => Address::ZERO,
                reth_rpc_types::trace::parity::Action::Selfdestruct(s) => s.address,
            },

            _ => unreachable!(),
        }
    }

    pub fn is_swap(&self) -> bool {
        matches!(self, Actions::Swap(_))
    }

    pub fn is_flash_loan(&self) -> bool {
        matches!(self, Actions::FlashLoan(_))
    }

    pub fn is_liquidation(&self) -> bool {
        matches!(self, Actions::Liquidation(_))
    }

    pub fn is_batch(&self) -> bool {
        matches!(self, Actions::Batch(_))
    }

    pub fn is_burn(&self) -> bool {
        matches!(self, Actions::Burn(_))
    }

    pub fn is_mint(&self) -> bool {
        matches!(self, Actions::Mint(_))
    }

    pub fn is_transfer(&self) -> bool {
        matches!(self, Actions::Transfer(_))
    }

    pub fn is_collect(&self) -> bool {
        matches!(self, Actions::Collect(_))
    }

    pub fn is_unclassified(&self) -> bool {
        matches!(self, Actions::Unclassified(_))
    }
}

#[derive(Debug, Serialize, Clone, Row, Deserialize)]
pub struct NormalizedFlashLoan {
    pub trace_index:       u64,
    pub from:              Address,
    pub pool:              Address,
    pub receiver_contract: Address,
    pub assets:            Vec<Address>,
    pub amounts:           Vec<U256>,
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
    pub fees_paid:     Vec<U256>,
}

impl NormalizedFlashLoan {
    pub fn finish_classification(&mut self, actions: Vec<(u64, Actions)>) -> Vec<u64> {
        let mut nodes_to_prune = Vec::new();
        let mut a_token_addresses = Vec::new();
        let mut repay_tranfers = Vec::new();

        for (index, action) in actions.into_iter() {
            match &action {
                // Use a reference to `action` here
                Actions::Swap(_)
                | Actions::FlashLoan(_)
                | Actions::Liquidation(_)
                | Actions::Batch(_)
                | Actions::Burn(_)
                | Actions::Mint(_) => {
                    self.child_actions.push(action);
                    nodes_to_prune.push(index);
                }
                Actions::Transfer(t) => {
                    // get the a_token reserve address that will be the receiver of the flashloan
                    // repayment for this token
                    if let Some(i) = self.assets.iter().position(|&x| x == t.token) {
                        if t.to == self.receiver_contract && t.amount == self.amounts[i] {
                            a_token_addresses.push(t.token);
                        }
                    }
                    // if the receiver contract is sending the token to the AToken address then this
                    // is the flashloan repayement
                    else if t.from == self.receiver_contract && a_token_addresses.contains(&t.to)
                    {
                        repay_tranfers.push(t.clone());
                        nodes_to_prune.push(index);
                    } else {
                        self.child_actions.push(action);
                        nodes_to_prune.push(index);
                    }
                }
                _ => continue,
            }
        }
        let mut fees = Vec::new();

        //TODO: deal with diff aave modes, where part of the flashloan is taken on as
        // debt by the OnBehalfOf address
        for (i, amount) in self.amounts.iter().enumerate() {
            let repay_amount = repay_tranfers
                .iter()
                .find(|t| t.token == self.assets[i])
                .map_or(U256::ZERO, |t| t.amount);
            let fee = repay_amount - amount;
            fees.push(fee);
        }

        self.fees_paid = fees;
        self.repayments = repay_tranfers;

        nodes_to_prune
    }
}

#[derive(Debug, Default, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedSwap {
    pub trace_index: u64,
    pub from:        Address,
    pub recipient:   Address,
    // If pool address is zero, then this is a p2p / CoW style swap, possibly within a batch
    pub pool:        Address,
    pub token_in:    Address,
    pub token_out:   Address,
    pub amount_in:   U256,
    pub amount_out:  U256,
}

#[derive(Debug, Default, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedBatch {
    pub trace_index:         u64,
    pub solver:              Address,
    pub settlement_contract: Address,
    pub user_swaps:          Vec<NormalizedSwap>,
    pub solver_swaps:        Option<Vec<NormalizedSwap>>,
}

#[derive(Debug, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedTransfer {
    pub trace_index: u64,
    pub to:          Address,
    pub from:        Address,
    pub token:       Address,
    pub amount:      U256,
}

#[derive(Debug, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedMint {
    pub trace_index: u64,
    pub from:        Address,
    pub to:          Address,
    pub recipient:   Address,
    pub token:       Vec<Address>,
    pub amount:      Vec<U256>,
}

#[derive(Debug, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedBurn {
    pub trace_index: u64,
    pub from:        Address,
    pub to:          Address,
    pub recipient:   Address,
    pub token:       Vec<Address>,
    pub amount:      Vec<U256>,
}

#[derive(Debug, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedCollect {
    pub trace_index: u64,
    pub to:          Address,
    pub from:        Address,
    pub recipient:   Address,
    pub token:       Vec<Address>,
    pub amount:      Vec<U256>,
}

#[derive(Debug, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedLiquidation {
    pub trace_index:      u64,
    pub pool:             Address,
    pub liquidator:       Address,
    pub debtor:           Address,
    pub collateral_asset: Address,
    pub debt_asset:       Address,
    pub amount:           U256,
}
#[derive(Debug, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedLoan {
    pub trace_index:  u64,
    pub lender:       Address,
    pub borrower:     Address,
    pub loaned_token: Address,
    pub loan_amount:  U256,
    pub collateral:   HashMap<Address, U256>,
}

#[derive(Debug, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedRepayment {
    pub trace_index:      u64,
    pub lender:           Address,
    pub borrower:         Address,
    pub repayed_token:    Address,
    pub repayment_amount: U256,
    pub collateral:       HashMap<Address, U256>,
}

pub trait NormalizedAction: Debug + Send + Sync + Clone {
    fn get_action(&self) -> &Actions;
    fn continue_classification(&self) -> bool;
    fn get_trace_index(&self) -> u64;
    fn continued_classification_types(&self) -> Box<dyn Fn(&Self) -> bool + Send + Sync>;
    fn finalize_classification(&mut self, actions: Vec<(u64, Self)>) -> Vec<u64>;
}

impl NormalizedAction for Actions {
    fn get_action(&self) -> &Actions {
        self
    }

    fn continue_classification(&self) -> bool {
        match self {
            Self::Swap(_) => false,
            Self::FlashLoan(_) => true,
            Self::Batch(_) => true,
            Self::Mint(_) => false,
            Self::Burn(_) => false,
            Self::Transfer(_) => false,
            Self::Liquidation(_) => true,
            Self::Collect(_) => false,
            Self::Unclassified(_) => false,
            Self::Revert => false,
            _ => unreachable!(),
        }
    }

    fn continued_classification_types(&self) -> Box<dyn Fn(&Self) -> bool + Send + Sync> {
        match self {
            Actions::Swap(_) => unreachable!(),
            Actions::FlashLoan(_) => Box::new(|action: &Actions| {
                action.is_liquidation()
                    | action.is_batch()
                    | action.is_swap()
                    | action.is_mint()
                    | action.is_burn()
                    | action.is_transfer()
                    | action.is_collect()
            }),
            Actions::Batch(_) => Box::new(|action: &Actions| action.is_swap() | action.is_burn()),
            Actions::Mint(_) => unreachable!(),
            Actions::Burn(_) => unreachable!(),
            Actions::Transfer(_) => unreachable!(),
            //TODO: Check later if it is possible to have nested liquidations
            Actions::Liquidation(_) => {
                Box::new(|action: &Actions| action.is_swap() | action.is_transfer())
            }
            Actions::Collect(_) => unreachable!(),
            Actions::Unclassified(_) => unreachable!(),
            _ => unreachable!(),
        }
    }

    fn get_trace_index(&self) -> u64 {
        match self {
            Self::Swap(s) => s.trace_index,
            Self::FlashLoan(f) => f.trace_index,
            Self::Batch(b) => b.trace_index,
            Self::Mint(m) => m.trace_index,
            Self::Burn(b) => b.trace_index,
            Self::Transfer(t) => t.trace_index,
            Self::Liquidation(t) => t.trace_index,
            Self::Collect(c) => c.trace_index,
            Self::Unclassified(u) => u.trace_idx,
            _ => unreachable!(),
        }
    }

    fn finalize_classification(&mut self, actions: Vec<(u64, Self)>) -> Vec<u64> {
        match self {
            Self::Swap(s) => unreachable!("Swap type never requires complex classification"),
            Self::FlashLoan(f) => f.finish_classification(actions),
            Self::Batch(b) => todo!(),
            Self::Mint(m) => unreachable!(),
            Self::Burn(b) => unreachable!(),
            Self::Transfer(t) => unreachable!(),
            Self::Liquidation(t) => todo!(),
            Self::Collect(c) => unreachable!("Collect type never requires complex classification"),
            Self::Unclassified(u) => {
                unreachable!("Unclassified type never requires complex classification")
            }
            _ => unreachable!(),
        }
    }
}
