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
use reth_rpc_types::trace::parity::Action;
pub use self_destruct::*;
use serde::{Deserialize, Serialize};
pub use swaps::*;
pub use transfer::*;

use self::pool::{NormalizedNewPool, NormalizedPoolConfigUpdate};
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
            Self::NewPool(_) => false,
            Self::PoolConfigUpdate(_) => false,
        }
    }

    fn continued_classification_types(&self) -> Box<dyn Fn(&Self) -> bool + Send + Sync> {
        match self {
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
            Actions::Liquidation(_) => Box::new(|action: &Actions| action.is_transfer()),
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
            Actions::NewPool(p) => p.trace_index,
            Actions::PoolConfigUpdate(p) => p.trace_index,
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

    pub fn force_transfer_mut(&mut self) -> &mut NormalizedTransfer {
        let Actions::Transfer(transfer) = self else {
            unreachable!("not transfer")
        };
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
                return Some(call.input.clone());
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
            return u.is_static_call();
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
