use std::{
    collections::HashMap,
    fmt::Debug,
    ops::{Deref, DerefMut},
};

use alloy_primitives::{Bytes, Log};
use reth_primitives::{Address, U256};
use reth_rpc_types::trace::parity::{Action, SelfdestructAction};
use serde::{Deserialize, Serialize};
use sorella_db_databases::{
    clickhouse,
    clickhouse::{DbRow, InsertRow, Row},
};

use crate::structured_trace::{TransactionTraceWithLogs, TraceActions};

/// A normalized action that has been classified
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub enum Actions {
    Swap(NormalizedSwap),
    SwapWithFee(NormalizedSwapWithFee),
    FlashLoan(NormalizedFlashLoan),
    Batch(NormalizedBatch),
    Transfer(NormalizedTransfer),
    Mint(NormalizedMint),
    Burn(NormalizedBurn),
    Collect(NormalizedCollect),
    Liquidation(NormalizedLiquidation),
    Unclassified(TransactionTraceWithLogs),
    SelfDestruct(SelfdestructWithIndex),
    Revert,
}

impl InsertRow for Actions {
    fn get_column_names(&self) -> &'static [&'static str] {
        match self {
            Actions::Swap(_) => NormalizedSwap::COLUMN_NAMES,
            Actions::SwapWithFee(_) => NormalizedSwapWithFee::COLUMN_NAMES,
            Actions::FlashLoan(_) => NormalizedFlashLoan::COLUMN_NAMES,
            Actions::Batch(_) => NormalizedBatch::COLUMN_NAMES,
            Actions::Transfer(_) => NormalizedTransfer::COLUMN_NAMES,
            Actions::Mint(_) => NormalizedMint::COLUMN_NAMES,
            Actions::Burn(_) => NormalizedBurn::COLUMN_NAMES,
            Actions::Collect(_) => NormalizedCollect::COLUMN_NAMES,
            Actions::Liquidation(_) => NormalizedLiquidation::COLUMN_NAMES,
            Actions::SelfDestruct(_) => todo!("joe pls dome this"),
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
            Actions::SwapWithFee(s) => s.serialize(serializer),
            Actions::FlashLoan(f) => f.serialize(serializer),
            Actions::Batch(b) => b.serialize(serializer),
            Actions::Mint(m) => m.serialize(serializer),
            Actions::Transfer(t) => t.serialize(serializer),
            Actions::Burn(b) => b.serialize(serializer),
            Actions::Collect(c) => c.serialize(serializer),
            Actions::Liquidation(c) => c.serialize(serializer),
            Actions::SelfDestruct(sd) => sd.serialize(serializer),
            Actions::Unclassified(trace) => (trace).serialize(serializer),
            _ => unreachable!(),
        }
    }
}

macro_rules! collect_action_fn {
    ($($action:ident),*) => {
        impl Actions {
            $(
                ::paste::paste!(
                    pub fn [<$action _collect_fn>]()
                    -> impl Fn(&crate::tree::Node<Self>) -> (bool, bool) {
                        |node | (node.data.[<is_ $action>](), node.get_all_sub_actions()
                                .iter().any(|i| i.[<is_ $action>]()))
                    }
                );
            )*
        }
    };
}

collect_action_fn!(
    swap,
    flash_loan,
    liquidation,
    batch,
    burn,
    revert,
    mint,
    transfer,
    collect,
    self_destruct,
    unclassified
);

impl Actions {
    pub fn force_liquidation(self) -> NormalizedLiquidation {
        match self {
            Actions::Liquidation(l) => l,
            _ => unreachable!(),
        }
    }

    pub fn force_swap(self) -> NormalizedSwap {
        match self {
            Actions::Swap(s) => s,
            Actions::SwapWithFee(s) => s.swap,
            _ => unreachable!(),
        }
    }

    pub fn force_swap_ref(&self) -> &NormalizedSwap {
        match self {
            Actions::Swap(s) => s,
            Actions::SwapWithFee(s) => s,
            _ => unreachable!(),
        }
    }

    pub fn force_swap_mut(&mut self) -> &mut NormalizedSwap {
        match self {
            Actions::Swap(s) => s,
            Actions::SwapWithFee(s) => s,
            _ => unreachable!(),
        }
    }

    pub fn get_logs(&self) -> Vec<Log> {
        match self {
            Self::Unclassified(a) => a.logs.clone(),
            _ => vec![],
        }
    }

    pub fn get_calldata(&self) -> Option<Bytes> {
        if let Actions::Unclassified(u) = &self {
            if let Action::Call(call) = &u.trace.action {
                return Some(call.input.clone())
            }
        }

        None
    }

    pub fn get_to_address(&self) -> Address {
        match self {
            Actions::Swap(s) => s.pool,
            Actions::SwapWithFee(s) => s.pool,
            Actions::FlashLoan(f) => f.pool,
            Actions::Batch(b) => b.settlement_contract,
            Actions::Mint(m) => m.to,
            Actions::Burn(b) => b.to,
            Actions::Transfer(t) => t.to,
            Actions::Collect(c) => c.to,
            Actions::Liquidation(c) => c.pool,
            Actions::SelfDestruct(c) => c.get_refund_address(),
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
        matches!(self, Actions::Swap(_)) || matches!(self, Actions::SwapWithFee(_))
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

    pub fn is_revert(&self) -> bool {
        matches!(self, Actions::Revert)
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

    pub fn is_self_destruct(&self) -> bool {
        matches!(self, Actions::SelfDestruct(_))
    }

    pub fn is_static_call(&self) -> bool {
        if let Self::Unclassified(u) = &self {
            return u.is_static_call()
        }
        false
    }

    pub fn is_unclassified(&self) -> bool {
        matches!(self, Actions::Unclassified(_))
    }
}

#[derive(Debug, Serialize, Clone, Row, Deserialize, PartialEq, Eq)]
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
pub struct NormalizedSwapWithFee {
    pub swap:       NormalizedSwap,
    pub fee_token:  Address,
    pub fee_amount: U256,
}

impl Deref for NormalizedSwapWithFee {
    type Target = NormalizedSwap;

    fn deref(&self) -> &Self::Target {
        &self.swap
    }
}
impl DerefMut for NormalizedSwapWithFee {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.swap
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

#[derive(Debug, Default, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedMint {
    pub trace_index: u64,
    pub from:        Address,
    pub to:          Address,
    pub recipient:   Address,
    pub token:       Vec<Address>,
    pub amount:      Vec<U256>,
}

#[derive(Debug, Default, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedBurn {
    pub trace_index: u64,
    pub from:        Address,
    pub to:          Address,
    pub recipient:   Address,
    pub token:       Vec<Address>,
    pub amount:      Vec<U256>,
}

#[derive(Debug, Default, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedCollect {
    pub trace_index: u64,
    pub to:          Address,
    pub from:        Address,
    pub recipient:   Address,
    pub token:       Vec<Address>,
    pub amount:      Vec<U256>,
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
pub struct NormalizedLiquidation {
    pub trace_index:           u64,
    pub pool:                  Address,
    pub liquidator:            Address,
    pub debtor:                Address,
    pub collateral_asset:      Address,
    pub debt_asset:            Address,
    pub covered_debt:          U256,
    pub liquidated_collateral: U256,
}

#[derive(Debug, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct SelfdestructWithIndex {
    pub trace_index:   u64,
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

impl NormalizedLiquidation {
    pub fn finish_classification(&mut self, actions: Vec<(u64, Actions)>) -> Vec<u64> {
        actions
            .into_iter()
            .find_map(|(index, action)| {
                if let Actions::Transfer(transfer) = action {
                    // because aave has the option to return the Atoken or regular,
                    // we can't filter by collateral filter. This might be an issue...
                    // tbd tho
                    if transfer.to == self.liquidator {
                        self.liquidated_collateral = transfer.amount;
                        return Some(index)
                    }
                }

                None
            })
            .map(|e| vec![e])
            .unwrap_or_default()
    }
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
            Self::SwapWithFee(_) => false,
            Self::FlashLoan(_) => true,
            Self::Batch(_) => true,
            Self::Mint(_) => false,
            Self::Burn(_) => false,
            Self::Transfer(_) => false,
            Self::Liquidation(_) => true,
            Self::Collect(_) => false,
            Self::SelfDestruct(_) => false,
            Self::Unclassified(_) => false,
            Self::Revert => false,
        }
    }

    fn continued_classification_types(&self) -> Box<dyn Fn(&Self) -> bool + Send + Sync> {
        match self {
            Actions::Swap(_) => unreachable!(),
            Actions::SwapWithFee(_) => unreachable!(),
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
            Actions::Liquidation(_) => Box::new(|action: &Actions| action.is_transfer()),
            Actions::Collect(_) => unreachable!(),
            Actions::SelfDestruct(_) => unreachable!(),
            Actions::Unclassified(_) => unreachable!(),
            Actions::Revert => unreachable!(),
        }
    }

    fn get_trace_index(&self) -> u64 {
        match self {
            Self::Swap(s) => s.trace_index,
            Self::SwapWithFee(s) => s.trace_index,
            Self::FlashLoan(f) => f.trace_index,
            Self::Batch(b) => b.trace_index,
            Self::Mint(m) => m.trace_index,
            Self::Burn(b) => b.trace_index,
            Self::Transfer(t) => t.trace_index,
            Self::Liquidation(t) => t.trace_index,
            Self::Collect(c) => c.trace_index,
            Self::SelfDestruct(c) => c.trace_index,
            Self::Unclassified(u) => u.trace_idx,
            Self::Revert => unreachable!(),
        }
    }

    fn finalize_classification(&mut self, actions: Vec<(u64, Self)>) -> Vec<u64> {
        match self {
            Self::Swap(_) => unreachable!("Swap type never requires complex classification"),
            Self::SwapWithFee(_) => {
                unreachable!("Swap With fee never requires complex classification")
            }
            Self::FlashLoan(f) => f.finish_classification(actions),
            Self::Batch(_) => todo!(),
            Self::Mint(_) => unreachable!(),
            Self::Burn(_) => unreachable!(),
            Self::Transfer(_) => unreachable!(),
            Self::Liquidation(l) => l.finish_classification(actions),
            Self::Collect(_) => unreachable!("Collect type never requires complex classification"),
            Self::SelfDestruct(_) => unreachable!(),
            Self::Unclassified(_) => {
                unreachable!("Unclassified type never requires complex classification")
            }
            Self::Revert => unreachable!("a revert should never require complex classification"),
        }
    }
}
