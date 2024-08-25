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
use reth_rpc_types::trace::parity::Action as TraceAction;
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
    fn get_action(&self) -> &Action;
    fn multi_frame_classification(&self) -> Option<MultiFrameRequest>;
    fn get_trace_index(&self) -> u64;
    fn is_create(&self) -> bool;
}

impl NormalizedAction for Action {
    fn is_create(&self) -> bool {
        if let Action::Unclassified(u) = self {
            u.is_create()
        } else {
            matches!(self, Action::NewPool(_))
        }
    }

    fn is_classified(&self) -> bool {
        !matches!(
            self,
            Action::Unclassified(_) | Action::EthTransfer(..) | Action::SelfDestruct(..)
        )
    }

    /// Only relevant for unclassified actions
    fn emitted_logs(&self) -> bool {
        match self {
            Action::Unclassified(u) => !u.logs.is_empty(),
            Action::SelfDestruct(_) => false,
            Action::EthTransfer(_) => false,
            _ => true,
        }
    }

    fn get_action(&self) -> &Action {
        self
    }

    fn multi_frame_classification(&self) -> Option<MultiFrameRequest> {
        MultiFrameRequest::new(self, self.try_get_trace_index()?)
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
pub enum Action {
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

impl InsertRow for Action {
    fn get_column_names(&self) -> &'static [&'static str] {
        match self {
            Action::Swap(_) => NormalizedSwap::COLUMN_NAMES,
            Action::SwapWithFee(_) => NormalizedSwapWithFee::COLUMN_NAMES,
            Action::FlashLoan(_) => NormalizedFlashLoan::COLUMN_NAMES,
            Action::Batch(_) => NormalizedBatch::COLUMN_NAMES,
            Action::Transfer(_) => NormalizedTransfer::COLUMN_NAMES,
            Action::Mint(_) => NormalizedMint::COLUMN_NAMES,
            Action::Burn(_) => NormalizedBurn::COLUMN_NAMES,
            Action::Collect(_) => NormalizedCollect::COLUMN_NAMES,
            Action::Liquidation(_) => NormalizedLiquidation::COLUMN_NAMES,
            Action::SelfDestruct(_) => todo!("joe pls dome this"),
            Action::EthTransfer(_) => todo!("joe pls dome this"),
            Action::NewPool(_) => todo!(),
            Action::PoolConfigUpdate(_) => todo!(),
            Action::Unclassified(..) | Action::Revert => panic!(),
            Action::Aggregator(_) => NormalizedAggregator::COLUMN_NAMES,
        }
    }
}

impl serde::Serialize for Action {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Action::Swap(s) => s.serialize(serializer),
            Action::SwapWithFee(s) => s.serialize(serializer),
            Action::FlashLoan(f) => f.serialize(serializer),
            Action::Aggregator(a) => a.serialize(serializer),
            Action::Batch(b) => b.serialize(serializer),
            Action::Mint(m) => m.serialize(serializer),
            Action::Transfer(t) => t.serialize(serializer),
            Action::Burn(b) => b.serialize(serializer),
            Action::Collect(c) => c.serialize(serializer),
            Action::Liquidation(c) => c.serialize(serializer),
            Action::SelfDestruct(sd) => sd.serialize(serializer),
            Action::EthTransfer(et) => et.serialize(serializer),
            Action::Unclassified(trace) => (trace).serialize(serializer),
            action => format!("{:?}", action).serialize(serializer),
            //action => unreachable!("no action serialization for {action:?}"),
        }
    }
}

impl Action {
    pub fn get_msg_value_not_eth_transfer(&self) -> Option<NormalizedEthTransfer> {
        let res =
            match self {
                Self::Swap(s) => (!s.msg_value.is_zero()).then(|| NormalizedEthTransfer {
                    value: s.msg_value,
                    to: s.pool,
                    from: s.from,
                    ..Default::default()
                }),
                Self::SwapWithFee(s) => (!s.msg_value.is_zero()).then(|| NormalizedEthTransfer {
                    value: s.msg_value,
                    to: s.pool,
                    from: s.from,
                    ..Default::default()
                }),

                Self::FlashLoan(f) => (!f.msg_value.is_zero()).then(|| NormalizedEthTransfer {
                    value: f.msg_value,
                    to: f.receiver_contract,
                    from: f.from,
                    ..Default::default()
                }),
                Self::Batch(b) => (!b.msg_value.is_zero()).then(|| NormalizedEthTransfer {
                    value: b.msg_value,
                    to: b.settlement_contract,
                    from: b.solver,
                    ..Default::default()
                }),
                Self::Liquidation(t) => (!t.msg_value.is_zero()).then(|| NormalizedEthTransfer {
                    value: t.msg_value,
                    to: t.pool,
                    from: t.liquidator,
                    ..Default::default()
                }),
                Self::Unclassified(u) => (!u.get_msg_value().is_zero() && !u.is_delegate_call())
                    .then(|| NormalizedEthTransfer {
                        value: u.get_msg_value(),
                        to: u.get_to_address(),
                        from: u.get_from_addr(),
                        ..Default::default()
                    }),
                Self::Aggregator(a) => (!a.msg_value.is_zero()).then(|| NormalizedEthTransfer {
                    value: a.msg_value,
                    to: a.to,
                    from: a.from,
                    ..Default::default()
                }),
                Self::Mint(_) => None,
                Self::Burn(_) => None,
                Self::Transfer(_) => None,
                Self::Collect(_) => None,
                Self::SelfDestruct(_) => None,
                Self::EthTransfer(_) => None,
                Self::NewPool(_) => None,
                Self::PoolConfigUpdate(_) => None,
                Self::Revert => None,
            };
        if res.is_some() {
            tracing::debug!(?res, ?self, "created eth transfer for internal accounting");
        }
        res
    }

    pub fn force_liquidation(self) -> NormalizedLiquidation {
        match self {
            Action::Liquidation(l) => l,
            _ => unreachable!("not liquidation"),
        }
    }

    fn try_get_trace_index(&self) -> Option<u64> {
        Some(match self {
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
            Self::Revert => return None,
        })
    }

    pub fn force_swap(self) -> NormalizedSwap {
        match self {
            Action::Swap(s) => s,
            Action::SwapWithFee(s) => s.swap,
            _ => unreachable!("not swap"),
        }
    }

    pub fn force_transfer(self) -> NormalizedTransfer {
        let Action::Transfer(transfer) = self else { unreachable!("not transfer") };
        transfer
    }

    pub fn force_transfer_mut(&mut self) -> &mut NormalizedTransfer {
        let Action::Transfer(transfer) = self else { unreachable!("not transfer") };
        transfer
    }

    pub fn force_swap_ref(&self) -> &NormalizedSwap {
        match self {
            Action::Swap(s) => s,
            Action::SwapWithFee(s) => s,
            _ => unreachable!("not swap"),
        }
    }

    pub fn force_swap_mut(&mut self) -> &mut NormalizedSwap {
        match self {
            Action::Swap(s) => s,
            Action::SwapWithFee(s) => s,
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
        if let Action::Unclassified(u) = &self {
            if let TraceAction::Call(call) = &u.trace.action {
                return Some(call.input.clone())
            }
        }

        None
    }

    pub fn get_to_address(&self) -> Address {
        match self {
            Action::Swap(s) => s.pool,
            Action::SwapWithFee(s) => s.pool,
            Action::FlashLoan(f) => f.pool,
            Action::Aggregator(a) => a.to,
            Action::Batch(b) => b.settlement_contract,
            Action::Mint(m) => m.pool,
            Action::Burn(b) => b.pool,
            Action::Transfer(t) => t.to,
            Action::Collect(c) => c.pool,
            Action::Liquidation(c) => c.pool,
            Action::SelfDestruct(c) => c.get_refund_address(),
            Action::Unclassified(t) => match &t.trace.action {
                reth_rpc_types::trace::parity::Action::Call(c) => c.to,
                reth_rpc_types::trace::parity::Action::Create(_) => Address::ZERO,
                reth_rpc_types::trace::parity::Action::Reward(_) => Address::ZERO,
                reth_rpc_types::trace::parity::Action::Selfdestruct(s) => s.address,
            },
            Action::EthTransfer(t) => t.to,
            Action::NewPool(p) => p.pool_address,
            Action::PoolConfigUpdate(p) => p.pool_address,
            Action::Revert => Address::ZERO,
        }
    }

    pub fn get_from_address(&self) -> Address {
        match self {
            Action::Swap(s) => s.from,
            Action::SwapWithFee(s) => s.from,
            Action::FlashLoan(f) => f.from,
            Action::Aggregator(a) => a.from,
            Action::Batch(b) => b.solver,
            Action::Mint(m) => m.from,
            Action::Burn(b) => b.from,
            Action::Transfer(t) => t.from,
            Action::Collect(c) => c.from,
            Action::Liquidation(c) => c.liquidator,
            Action::SelfDestruct(c) => c.get_address(),
            Action::Unclassified(t) => match &t.trace.action {
                reth_rpc_types::trace::parity::Action::Call(c) => c.to,
                reth_rpc_types::trace::parity::Action::Create(_) => Address::ZERO,
                reth_rpc_types::trace::parity::Action::Reward(_) => Address::ZERO,
                reth_rpc_types::trace::parity::Action::Selfdestruct(s) => s.address,
            },
            Action::EthTransfer(t) => t.from,
            Action::Revert => unreachable!(),
            Action::NewPool(_) => Address::ZERO,
            Action::PoolConfigUpdate(_) => Address::ZERO,
        }
    }

    pub const fn is_nested_action(&self) -> bool {
        matches!(self, Action::FlashLoan(_))
            || matches!(self, Action::Batch(_))
            || matches!(self, Action::Aggregator(_))
    }

    pub const fn is_swap(&self) -> bool {
        matches!(self, Action::Swap(_)) || matches!(self, Action::SwapWithFee(_))
    }

    pub const fn is_swap_no_fee(&self) -> bool {
        matches!(self, Action::Swap(_))
    }

    pub const fn is_swap_with_fee(&self) -> bool {
        matches!(self, Action::SwapWithFee(_))
    }

    pub const fn is_flash_loan(&self) -> bool {
        matches!(self, Action::FlashLoan(_))
    }

    pub const fn is_aggregator(&self) -> bool {
        matches!(self, Action::Aggregator(_))
    }

    pub const fn is_liquidation(&self) -> bool {
        matches!(self, Action::Liquidation(_))
    }

    pub const fn is_batch(&self) -> bool {
        matches!(self, Action::Batch(_))
    }

    pub const fn is_burn(&self) -> bool {
        matches!(self, Action::Burn(_))
    }

    pub const fn is_revert(&self) -> bool {
        matches!(self, Action::Revert)
    }

    pub const fn is_mint(&self) -> bool {
        matches!(self, Action::Mint(_))
    }

    pub const fn is_transfer(&self) -> bool {
        matches!(self, Action::Transfer(_))
    }

    pub const fn is_collect(&self) -> bool {
        matches!(self, Action::Collect(_))
    }

    pub const fn is_self_destruct(&self) -> bool {
        matches!(self, Action::SelfDestruct(_))
    }

    pub const fn is_new_pool(&self) -> bool {
        matches!(self, Action::NewPool(_))
    }

    pub const fn is_pool_config_update(&self) -> bool {
        matches!(self, Action::PoolConfigUpdate(_))
    }

    pub const fn is_unclassified(&self) -> bool {
        matches!(self, Action::Unclassified(_))
    }

    pub const fn get_protocol(&self) -> Protocol {
        match self {
            Action::Swap(s) => s.protocol,
            Action::SwapWithFee(s) => s.swap.protocol,
            Action::FlashLoan(f) => f.protocol,
            Action::Batch(b) => b.protocol,
            Action::Mint(m) => m.protocol,
            Action::Burn(b) => b.protocol,
            Action::Collect(c) => c.protocol,
            Action::Liquidation(c) => c.protocol,
            Action::NewPool(p) => p.protocol,
            Action::PoolConfigUpdate(p) => p.protocol,
            Action::Aggregator(a) => a.protocol,
            _ => Protocol::Unknown,
        }
    }

    pub const fn is_eth_transfer(&self) -> bool {
        matches!(self, Action::EthTransfer(_))
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

            impl Action {
                $(
                    pub fn [<try _$action_name:snake _ref>](&self) -> Option<&$ret> {
                        if let Action::$action_name(action) = self {
                            Some(action)
                        } else {
                            None
                        }
                    }

                    pub fn [<try _$action_name:snake _mut>](&mut self) -> Option<&mut $ret> {
                        if let Action::$action_name(action) = self {
                            Some(action)
                        } else {
                            None
                        }
                    }

                    pub fn [<try _$action_name:snake>](self) -> Option<$ret> {
                        if let Action::$action_name(action) = self {
                            Some(action)
                        } else {
                            None
                        }
                    }

                    pub fn [<try _$action_name:snake _dedup>]()
                        -> Box<dyn Fn(Action) -> Option<$ret>> {
                        Box::new(Action::[<try _$action_name:snake>])
                                as Box<dyn Fn(Action) -> Option<$ret>>
                    }
                )*
            }

            $(
                impl From<$ret> for Action {
                    fn from(value: $ret) -> Action {
                        Action::$action_name(value)
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
    (Batch, NormalizedBatch),
    (NewPool, NormalizedNewPool)
);

/// Custom impl for itering over swaps and swap with fee
impl Action {
    /// Merges swap and swap with fee
    pub fn try_swaps_merged_ref(&self) -> Option<&NormalizedSwap> {
        match self {
            Action::Swap(action) => Some(action),
            Action::SwapWithFee(f) => Some(f),
            _ => None,
        }
    }

    /// Merges swap and swap with fee
    pub fn try_swaps_merged_mut(&mut self) -> Option<&mut NormalizedSwap> {
        match self {
            Action::Swap(action) => Some(action),
            Action::SwapWithFee(f) => Some(f),
            _ => None,
        }
    }

    /// Merges swap and swap with fee
    pub fn try_swaps_merged(self) -> Option<NormalizedSwap> {
        match self {
            Action::Swap(action) => Some(action),
            Action::SwapWithFee(f) => Some(f.swap),
            _ => None,
        }
    }

    /// Merges swap and swap with fee
    pub fn try_swaps_merged_dedup() -> Box<dyn Fn(Action) -> Option<NormalizedSwap>> {
        Box::new(Action::try_swaps_merged) as Box<dyn Fn(Action) -> Option<NormalizedSwap>>
    }
}

impl TokenAccounting for Action {
    fn apply_token_deltas(&self, delta_map: &mut AddressDeltas) {
        match self {
            Action::Swap(swap) => swap.apply_token_deltas(delta_map),
            Action::Transfer(transfer) => transfer.apply_token_deltas(delta_map),
            Action::FlashLoan(flash_loan) => flash_loan.apply_token_deltas(delta_map),
            Action::Aggregator(aggregator) => aggregator.apply_token_deltas(delta_map),
            Action::Liquidation(liquidation) => liquidation.apply_token_deltas(delta_map),
            Action::Batch(batch) => batch.apply_token_deltas(delta_map),
            Action::Burn(burn) => burn.apply_token_deltas(delta_map),
            Action::Mint(mint) => mint.apply_token_deltas(delta_map),
            Action::SwapWithFee(swap_with_fee) => swap_with_fee.swap.apply_token_deltas(delta_map),
            Action::Collect(collect) => collect.apply_token_deltas(delta_map),
            Action::EthTransfer(eth_transfer) => eth_transfer.apply_token_deltas(delta_map),
            Action::Unclassified(_) => (), /* Potentially no token deltas to apply, adjust as */
            // necessary
            Action::SelfDestruct(_self_destruct) => (),
            Action::NewPool(_new_pool) => (),
            Action::PoolConfigUpdate(_pool_update) => (),
            Action::Revert => (), // No token deltas to apply for a revert
        }
    }
}
