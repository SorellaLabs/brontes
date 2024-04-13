pub mod accounting;
pub mod aggregator;
pub mod batch;
pub mod comparison;
pub mod eth_transfer;
pub mod flashloan;
pub mod lending;
pub mod liquidation;
pub mod liquidity;
pub mod multi_callframe;
pub mod pool;
pub mod self_destruct;
pub mod swaps;
pub mod transfer;
use std::fmt::Debug;

use ::clickhouse::DbRow;
use accounting::{AddressDeltas, TokenAccounting};
pub use aggregator::*;
use alloy_primitives::{Address, Bytes, Log};
pub use batch::*;
use clickhouse::InsertRow;
pub use eth_transfer::*;
pub use flashloan::*;
pub use lending::*;
pub use liquidation::*;
pub use liquidity::*;
pub use multi_callframe::*;
pub use pool::*;
use reth_rpc_types::trace::parity::Action;
pub use self_destruct::*;
pub use swaps::*;
pub use transfer::*;

use crate::{
    structured_trace::{TraceActions, TransactionTraceWithLogs},
    Protocol,
};

pub trait NormalizedAction: Debug + Send + Sync + Clone + PartialEq + Eq {
    fn is_classified(&self) -> bool;
    fn emitted_logs(&self) -> bool;
    fn get_action(&self) -> &Actions;
    fn multi_frame_classification(&self, trace_idx: u64) -> Option<MultiFrameRequest>;
    fn get_trace_index(&self) -> u64;
}

impl NormalizedAction for Actions {
    fn is_classified(&self) -> bool {
        !matches!(
            self,
            Actions::Unclassified(_) | Actions::EthTransfer(..) | Actions::SelfDestruct(..)
        )
    }

    /// Only relevant for unclassified actions
    fn emitted_logs(&self) -> bool {
        match self {
            Actions::Unclassified(u) => !u.logs.is_empty(),
            Actions::SelfDestruct(_) => false,
            Actions::EthTransfer(_) => false,
            _ => true,
        }
    }

    fn get_action(&self) -> &Actions {
        self
    }

    fn multi_frame_classification(&self, trace_idx: u64) -> Option<MultiFrameRequest> {
        MultiFrameRequest::new(self, trace_idx)
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
            Self::Aggregator(a) => a.trace_index,
            Self::Revert => unreachable!("no trace index for revert"),
        }
    }
}

/// A normalized action that has been classified
#[derive(Debug, Clone, serde::Deserialize, PartialEq, Eq)]
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
    SelfDestruct(SelfdestructWithIndex),
    EthTransfer(NormalizedEthTransfer),
    NewPool(NormalizedNewPool),
    PoolConfigUpdate(NormalizedPoolConfigUpdate),
    Aggregator(NormalizedAggregator),
    Unclassified(TransactionTraceWithLogs),
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
            Actions::Aggregator(_) => NormalizedAggregator::COLUMN_NAMES,
        }
    }
}

impl serde::Serialize for Actions {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Actions::Swap(s) => s.serialize(serializer),
            Actions::SwapWithFee(s) => s.serialize(serializer),
            Actions::FlashLoan(f) => f.serialize(serializer),
            Actions::Aggregator(a) => a.serialize(serializer),
            Actions::Batch(b) => b.serialize(serializer),
            Actions::Mint(m) => m.serialize(serializer),
            Actions::Transfer(t) => t.serialize(serializer),
            Actions::Burn(b) => b.serialize(serializer),
            Actions::Collect(c) => c.serialize(serializer),
            Actions::Liquidation(c) => c.serialize(serializer),
            Actions::SelfDestruct(sd) => sd.serialize(serializer),
            Actions::EthTransfer(et) => et.serialize(serializer),
            Actions::Unclassified(trace) => (trace).serialize(serializer),
            action => format!("{:?}", action).serialize(serializer),
            //action => unreachable!("no action serialization for {action:?}"),
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
            Actions::Aggregator(_) => Address::ZERO,
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
            Actions::Aggregator(a) => a.from,
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

    pub const fn is_aggregator(&self) -> bool {
        matches!(self, Actions::Aggregator(_))
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

    pub const fn is_pool_config_update(&self) -> bool {
        matches!(self, Actions::PoolConfigUpdate(_))
    }

    pub const fn is_unclassified(&self) -> bool {
        matches!(self, Actions::Unclassified(_))
    }

    pub const fn get_protocol(&self) -> Protocol {
        match self {
            Actions::Swap(s) => s.protocol,
            Actions::SwapWithFee(s) => s.swap.protocol,
            Actions::FlashLoan(f) => f.protocol,
            Actions::Batch(b) => b.protocol,
            Actions::Mint(m) => m.protocol,
            Actions::Burn(b) => b.protocol,
            Actions::Collect(c) => c.protocol,
            Actions::Liquidation(c) => c.protocol,
            Actions::NewPool(p) => p.protocol,
            Actions::PoolConfigUpdate(p) => p.protocol,
            Actions::Aggregator(a) => a.protocol,
            _ => Protocol::Unknown,
        }
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
                )*
            }

            $(
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
    (EthTransfer, NormalizedEthTransfer),
    (Liquidation, NormalizedLiquidation),
    (FlashLoan, NormalizedFlashLoan),
    (Aggregator, NormalizedAggregator),
    (Batch, NormalizedBatch)
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

impl TokenAccounting for Actions {
    fn apply_token_deltas(&self, delta_map: &mut AddressDeltas) {
        match self {
            Actions::Swap(swap) => swap.apply_token_deltas(delta_map),
            Actions::Transfer(transfer) => transfer.apply_token_deltas(delta_map),
            Actions::FlashLoan(flash_loan) => flash_loan.apply_token_deltas(delta_map),
            Actions::Aggregator(aggregator) => aggregator.apply_token_deltas(delta_map),
            Actions::Liquidation(liquidation) => liquidation.apply_token_deltas(delta_map),
            Actions::Batch(batch) => batch.apply_token_deltas(delta_map),
            Actions::Burn(burn) => burn.apply_token_deltas(delta_map),
            Actions::Mint(mint) => mint.apply_token_deltas(delta_map),
            Actions::SwapWithFee(swap_with_fee) => swap_with_fee.swap.apply_token_deltas(delta_map),
            Actions::Collect(collect) => collect.apply_token_deltas(delta_map),
            Actions::EthTransfer(eth_transfer) => eth_transfer.apply_token_deltas(delta_map),
            Actions::Unclassified(_) => (), /* Potentially no token deltas to apply, adjust as */
            // necessary
            Actions::SelfDestruct(_self_destruct) => (),
            Actions::NewPool(_new_pool) => (),
            Actions::PoolConfigUpdate(_pool_update) => (),
            Actions::Revert => (), // No token deltas to apply for a revert
        }
    }
}
