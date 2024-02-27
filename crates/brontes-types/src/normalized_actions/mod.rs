pub mod batch;
pub mod eth_transfer;
pub mod flashloan;
pub mod lending;
pub mod liquidation;
pub mod liquidity;
pub mod pool;
pub mod self_destruct;
pub mod swaps;
pub mod transfer;
pub mod utils;
use std::fmt::Debug;

use ::clickhouse::DbRow;
use alloy_primitives::{Address, Bytes, Log};
pub use batch::*;
use clickhouse::InsertRow;
pub use eth_transfer::*;
pub use flashloan::*;
pub use lending::*;
pub use liquidation::*;
pub use liquidity::*;
pub use pool::*;
use reth_rpc_types::trace::parity::Action;
pub use self_destruct::*;
use serde::{Deserialize, Serialize};
pub use swaps::*;
pub use transfer::*;

use crate::{
    structured_trace::{TraceActions, TransactionTraceWithLogs},
    TreeSearchBuilder,
};

pub trait NormalizedAction: Debug + Send + Sync + Clone + PartialEq + Eq {
    fn is_classified(&self) -> bool;
    fn emitted_logs(&self) -> bool;
    fn get_action(&self) -> &Actions;
    fn continue_classification(&self) -> bool;
    fn get_trace_index(&self) -> u64;
    fn continued_classification_types(&self) -> TreeSearchBuilder<Self>;
    fn finalize_classification(&mut self, actions: Vec<(u64, Self)>) -> Vec<u64>;
}

impl NormalizedAction for Actions {
    fn is_classified(&self) -> bool {
        !matches!(self, Actions::Unclassified(_))
    }

    /// Only relevant for unclassified actions
    fn emitted_logs(&self) -> bool {
        match self {
            Actions::Unclassified(u) => !u.logs.is_empty(),
            _ => true,
        }
    }

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
            Self::EthTransfer(_) => false,
            Self::Unclassified(_) => false,
            Self::Revert => false,
            Self::NewPool(_) => false,
            Self::PoolConfigUpdate(_) => false,
        }
    }

    fn continued_classification_types(&self) -> TreeSearchBuilder<Self> {
        match self {
            Self::FlashLoan(_) => TreeSearchBuilder::default().with_actions([
                Self::is_batch,
                Self::is_swap,
                Self::is_liquidation,
                Self::is_mint,
                Self::is_burn,
                Self::is_transfer,
                Self::is_collect,
            ]),
            Self::Batch(_) => TreeSearchBuilder::default().with_actions([
                Self::is_swap,
                Self::is_transfer,
                Self::is_eth_transfer,
            ]),
            Self::Liquidation(_) => TreeSearchBuilder::default().with_action(Self::is_transfer),
            action => unreachable!("no continue_classification function for {action:?}"),
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
            Self::EthTransfer(e) => e.trace_index,
            Self::Unclassified(u) => u.trace_idx,
            Self::NewPool(p) => p.trace_index,
            Self::PoolConfigUpdate(p) => p.trace_index,
            Self::Revert => unreachable!("no trace index for revert"),
        }
    }

    fn finalize_classification(&mut self, actions: Vec<(u64, Self)>) -> Vec<u64> {
        match self {
            Self::FlashLoan(f) => f.finish_classification(actions),
            Self::Batch(f) => f.finish_classification(actions),
            Self::Liquidation(l) => l.finish_classification(actions),
            action => unreachable!("{action:?} never require complex classification"),
        }
    }
}

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
    EthTransfer(NormalizedEthTransfer),
    NewPool(NormalizedNewPool),
    PoolConfigUpdate(NormalizedPoolConfigUpdate),
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
            Actions::EthTransfer(_) => todo!("joe pls dome this"),
            Actions::NewPool(_) => todo!(),
            Actions::PoolConfigUpdate(_) => todo!(),
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
            Actions::EthTransfer(et) => et.serialize(serializer),
            Actions::Unclassified(trace) => (trace).serialize(serializer),
            action => unreachable!("no action serialization for {action:?}"),
        }
    }
}

impl Actions {
    pub fn force_liquidation(self) -> NormalizedLiquidation {
        match self {
            Actions::Liquidation(l) => l,
            _ => unreachable!("not liquidation"),
        }
    }

    pub fn force_swap(self) -> NormalizedSwap {
        match self {
            Actions::Swap(s) => s,
            Actions::SwapWithFee(s) => s.swap,
            _ => unreachable!("not swap"),
        }
    }

    pub fn force_transfer(self) -> NormalizedTransfer {
        let Actions::Transfer(transfer) = self else { unreachable!("not transfer") };
        transfer
    }

    pub fn force_transfer_mut(&mut self) -> &mut NormalizedTransfer {
        let Actions::Transfer(transfer) = self else { unreachable!("not transfer") };
        transfer
    }

    pub fn force_swap_ref(&self) -> &NormalizedSwap {
        match self {
            Actions::Swap(s) => s,
            Actions::SwapWithFee(s) => s,
            _ => unreachable!("not swap"),
        }
    }

    pub fn force_swap_mut(&mut self) -> &mut NormalizedSwap {
        match self {
            Actions::Swap(s) => s,
            Actions::SwapWithFee(s) => s,
            _ => unreachable!("not swap"),
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
            Actions::Mint(m) => m.pool,
            Actions::Burn(b) => b.pool,
            Actions::Transfer(t) => t.to,
            Actions::Collect(c) => c.pool,
            Actions::Liquidation(c) => c.pool,
            Actions::SelfDestruct(c) => c.get_refund_address(),
            Actions::Unclassified(t) => match &t.trace.action {
                reth_rpc_types::trace::parity::Action::Call(c) => c.to,
                reth_rpc_types::trace::parity::Action::Create(_) => Address::ZERO,
                reth_rpc_types::trace::parity::Action::Reward(_) => Address::ZERO,
                reth_rpc_types::trace::parity::Action::Selfdestruct(s) => s.address,
            },
            Actions::EthTransfer(t) => t.to,
            Actions::NewPool(p) => p.pool_address,
            Actions::PoolConfigUpdate(p) => p.pool_address,
            Actions::Revert => Address::ZERO,
        }
    }

    pub fn get_from_address(&self) -> Address {
        match self {
            Actions::Swap(s) => s.from,
            Actions::SwapWithFee(s) => s.from,
            Actions::FlashLoan(f) => f.from,
            Actions::Batch(b) => b.solver,
            Actions::Mint(m) => m.from,
            Actions::Burn(b) => b.from,
            Actions::Transfer(t) => t.from,
            Actions::Collect(c) => c.from,
            Actions::Liquidation(c) => c.liquidator,
            Actions::SelfDestruct(c) => c.get_address(),
            Actions::Unclassified(t) => match &t.trace.action {
                reth_rpc_types::trace::parity::Action::Call(c) => c.to,
                reth_rpc_types::trace::parity::Action::Create(_) => Address::ZERO,
                reth_rpc_types::trace::parity::Action::Reward(_) => Address::ZERO,
                reth_rpc_types::trace::parity::Action::Selfdestruct(s) => s.address,
            },
            Actions::EthTransfer(t) => t.from,
            Actions::Revert => unreachable!(),
            Actions::NewPool(_) => Address::ZERO,
            Actions::PoolConfigUpdate(_) => Address::ZERO,
        }
    }

    pub const fn is_swap(&self) -> bool {
        matches!(self, Actions::Swap(_)) || matches!(self, Actions::SwapWithFee(_))
    }

    pub const fn is_swap_no_fee(&self) -> bool {
        matches!(self, Actions::Swap(_))
    }

    pub const fn is_swap_with_fee(&self) -> bool {
        matches!(self, Actions::SwapWithFee(_))
    }

    pub const fn is_flash_loan(&self) -> bool {
        matches!(self, Actions::FlashLoan(_))
    }

    pub const fn is_liquidation(&self) -> bool {
        matches!(self, Actions::Liquidation(_))
    }

    pub const fn is_batch(&self) -> bool {
        matches!(self, Actions::Batch(_))
    }

    pub const fn is_burn(&self) -> bool {
        matches!(self, Actions::Burn(_))
    }

    pub const fn is_revert(&self) -> bool {
        matches!(self, Actions::Revert)
    }

    pub const fn is_mint(&self) -> bool {
        matches!(self, Actions::Mint(_))
    }

    pub const fn is_transfer(&self) -> bool {
        matches!(self, Actions::Transfer(_))
    }

    pub const fn is_collect(&self) -> bool {
        matches!(self, Actions::Collect(_))
    }

    pub const fn is_self_destruct(&self) -> bool {
        matches!(self, Actions::SelfDestruct(_))
    }

    pub const fn is_new_pool(&self) -> bool {
        matches!(self, Actions::NewPool(_))
    }

    pub const fn is_unclassified(&self) -> bool {
        matches!(self, Actions::Unclassified(_))
    }

    pub const fn is_eth_transfer(&self) -> bool {
        matches!(self, Actions::EthTransfer(_))
    }

    pub fn is_static_call(&self) -> bool {
        if let Self::Unclassified(u) = &self {
            return u.is_static_call()
        }
        false
    }
}

macro_rules! extra_impls {
    ($(($action_name:ident, $ret:ident)),*) => {
        paste::paste!(

            impl Actions {
                $(
                    pub fn [<try _$action_name:snake _ref>](&self) -> Option<&$ret> {
                        if let Actions::$action_name(action) = self {
                            Some(action)
                        } else {
                            None
                        }
                    }

                    pub fn [<try _$action_name:snake _mut>](&mut self) -> Option<&mut $ret> {
                        if let Actions::$action_name(action) = self {
                            Some(action)
                        } else {
                            None
                        }
                    }

                    pub fn [<try _$action_name:snake>](self) -> Option<$ret> {
                        if let Actions::$action_name(action) = self {
                            Some(action)
                        } else {
                            None
                        }
                    }

                    pub fn [<try _$action_name:snake _dedup>]()
                        -> Box<dyn Fn(Actions) -> Option<$ret>> {
                        Box::new(Actions::[<try _$action_name:snake>])
                                as Box<dyn Fn(Actions) -> Option<$ret>>
                    }

                    pub fn [<$action_name:snake key>]() -> ActionsKey<$ret>{
                        ActionsKey {
                            matches_ptr: Actions::[<is _$action_name>],
                            into_ptr: Actions::[<try _$action_name:snake>]
                        }
                    }

                )*
            }

            $(
                impl NormalizedActionKey<Actions> for $ret {
                    type Out = $ret;
                    fn get_key(&self) -> ActionsKey<Self::Out> {
                        Actions::[<$action_name:snake key>]()
                    }
                }

                impl From<$ret> for Actions {
                    fn from(value: $ret) -> Actions {
                        Actions::$action_name(value)
                    }
                }
            )*
        );

    };
}

extra_impls!(
    (Collect, NormalizedCollect),
    (Mint, NormalizedMint),
    (Burn, NormalizedBurn),
    (Swap, NormalizedSwap),
    (SwapWithFee, NormalizedSwapWithFee),
    (Transfer, NormalizedTransfer),
    (Liquidation, NormalizedLiquidation),
    (FlashLoan, NormalizedFlashLoan)
);

/// Custom impl for itering over swaps and swap with fee
impl Actions {
    /// Merges swap and swap with fee
    pub fn try_swaps_merged_ref(&self) -> Option<&NormalizedSwap> {
        match self {
            Actions::Swap(action) => Some(action),
            Actions::SwapWithFee(f) => Some(f),
            _ => None,
        }
    }

    /// Merges swap and swap with fee
    pub fn try_swaps_merged_mut(&mut self) -> Option<&mut NormalizedSwap> {
        match self {
            Actions::Swap(action) => Some(action),
            Actions::SwapWithFee(f) => Some(f),
            _ => None,
        }
    }

    /// Merges swap and swap with fee
    pub fn try_swaps_merged(self) -> Option<NormalizedSwap> {
        match self {
            Actions::Swap(action) => Some(action),
            Actions::SwapWithFee(f) => Some(f.swap),
            _ => None,
        }
    }

    /// Merges swap and swap with fee
    pub fn try_swaps_merged_dedup() -> Box<dyn Fn(Actions) -> Option<NormalizedSwap>> {
        Box::new(Actions::try_swaps_merged) as Box<dyn Fn(Actions) -> Option<NormalizedSwap>>
    }
}

#[derive(PartialEq, Eq)]
pub struct ActionsKey<O: PartialEq + Eq> {
    matches_ptr: fn(&Actions) -> bool,
    into_ptr:    fn(Actions) -> Option<O>,
}

impl NormalizedActionKey<Actions> for Actions {
    type Out = ();

    fn get_key(&self) -> ActionsKey<Self::Out> {
        match self {
            _ => todo!(),
        }
    }

    // fn matches(&self, other: &Actions) -> bool {
    //     self.matches_ptr(other)
    // }
    //
    // fn into_val(&self, item: Actions) -> O {
    //     self.into_ptr(item)
    //         .expect("into ptr should never be none, this means the data wasn't
    // checked against") }
}

pub trait NormalizedActionKey<V: NormalizedAction>: PartialEq + Eq {
    type Out: PartialEq + Eq;
    fn get_key(&self) -> ActionsKey<Self::Out>;
}
