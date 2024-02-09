pub mod batch;
pub mod eth_transfer;
pub mod flashloan;
pub mod lending;
pub mod liquidation;
pub mod liquidity;
pub mod self_destruct;
pub mod swaps;
pub mod transfer;
use std::fmt::Debug;

use alloy_primitives::{Address, Bytes, Log};
pub use batch::*;
pub use eth_transfer::*;
pub use flashloan::*;
pub use lending::*;
pub use liquidation::*;
pub use liquidity::*;
use reth_rpc_types::trace::parity::Action;
pub use self_destruct::*;
use serde::{Deserialize, Serialize};
use sorella_db_databases::clickhouse::{DbRow, InsertRow};
pub use swaps::*;
pub use transfer::*;

use crate::structured_trace::{TraceActions, TransactionTraceWithLogs};

pub trait NormalizedAction: Debug + Send + Sync + Clone {
    fn is_classified(&self) -> bool;
    fn get_action(&self) -> &Actions;
    fn continue_classification(&self) -> bool;
    fn get_trace_index(&self) -> u64;
    fn continued_classification_types(&self) -> Box<dyn Fn(&Self) -> bool + Send + Sync>;
    fn finalize_classification(&mut self, actions: Vec<(u64, Self)>) -> Vec<u64>;
}

impl NormalizedAction for Actions {
    fn is_classified(&self) -> bool {
        !matches!(self, Actions::Unclassified(_))
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
            Actions::Batch(_) => Box::new(|action: &Actions| {
                action.is_swap() | action.is_transfer() | action.is_eth_transfer()
            }),
            Actions::Mint(_) => unreachable!(),
            Actions::Burn(_) => unreachable!(),
            Actions::Transfer(_) => unreachable!(),
            Actions::Liquidation(_) => Box::new(|action: &Actions| action.is_transfer()),
            Actions::Collect(_) => unreachable!(),
            Actions::SelfDestruct(_) => unreachable!(),
            Actions::EthTransfer(_) => unreachable!(),
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
            Self::EthTransfer(e) => e.trace_index,
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
            Self::Batch(f) => f.finish_classification(actions),
            Self::Mint(_) => unreachable!(),
            Self::Burn(_) => unreachable!(),
            Self::Transfer(_) => unreachable!(),
            Self::Liquidation(l) => l.finish_classification(actions),
            Self::Collect(_) => unreachable!("Collect type never requires complex classification"),
            Self::SelfDestruct(_) => unreachable!(),
            Self::EthTransfer(_) => unreachable!(),
            Self::Unclassified(_) => {
                unreachable!("Unclassified type never requires complex classification")
            }
            Self::Revert => unreachable!("a revert should never require complex classification"),
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
    unclassified,
    eth_transfer
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

    pub fn force_transfer_mut(&mut self) -> &mut NormalizedTransfer {
        let Actions::Transfer(transfer) = self else { unreachable!() };
        transfer
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
            Actions::Revert => unreachable!(),
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

    pub fn is_eth_transfer(&self) -> bool {
        matches!(self, Actions::EthTransfer(_))
    }
}
