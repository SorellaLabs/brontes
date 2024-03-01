use std::fmt::Debug;

use super::{Actions, NormalizedCollect, NormalizedMint, NormalizedSwap, NormalizedTransfer};

impl<T: Sized + SubordinateAction<O>, O: ActionCmp<T>> ActionComparison<O> for T {}

pub trait ActionComparison<O> {
    fn is_same_coverage(&self, other: &O) -> bool
    where
        Self: Sized + ActionCmp<O> + Debug,
        O: ActionCmp<Self> + Debug,
    {
        self.is_superior_action(other) || other.is_subordinate(self)
    }
}

impl ActionCmp<Actions> for Actions {
    fn is_superior_action(&self, other: &Actions) -> bool {
        match self {
            Actions::Swap(s) => s.is_superior_action(other),
            Actions::Mint(m) => m.is_superior_action(other),
            Actions::Collect(c) => c.is_superior_action(other),
            Actions::SwapWithFee(s) => s.swap.is_superior_action(other),
            Actions::FlashLoan(f) => f.child_actions.iter().any(|a| a.is_superior_action(other)),
            Actions::Batch(b) => {
                let user = b.user_swaps.iter().any(|b| b.is_superior_action(other));
                if let Some(swaps) = &b.solver_swaps {
                    return user || swaps.iter().any(|b| b.is_superior_action(other))
                }
                user
            }

            action => {
                tracing::debug!(?action, ?other, "no action cmp impl for given action");
                false
            }
        }
    }
}
/// For two actions, will tell you if the actions is the more superior action (a
/// swap is superior to a transfer of a swap)
pub trait ActionCmp<O> {
    /// checks if this action is the superior action. eg Swap is the superior
    /// action to a transfer related to the swap
    fn is_superior_action(&self, other: &O) -> bool;
}

impl<T: Sized, O: ActionCmp<T>> SubordinateAction<O> for T {}

pub trait SubordinateAction<O> {
    /// checks to see if this action is subordinate to the other action.
    fn is_subordinate(&self, other: &O) -> bool
    where
        Self: Sized + Debug,
        O: ActionCmp<Self> + Debug,
    {
        other.is_superior_action(self)
    }
}

impl ActionCmp<NormalizedTransfer> for NormalizedSwap {
    fn is_superior_action(&self, transfer: &NormalizedTransfer) -> bool {
        tracing::info!("seeing if we need to dedup swap transfer");
        (&transfer.amount + &transfer.fee == self.amount_in
            && transfer.to == self.pool
            && self.from == transfer.from)
            || (transfer.amount == self.amount_out
                && transfer.from == self.pool
                && self.recipient == transfer.to)
    }
}

impl ActionCmp<Actions> for NormalizedSwap {
    fn is_superior_action(&self, other: &Actions) -> bool {
        match other {
            Actions::Transfer(t) => self.is_superior_action(t),
            _ => false,
        }
    }
}

impl ActionCmp<NormalizedTransfer> for NormalizedMint {
    fn is_superior_action(&self, transfer: &NormalizedTransfer) -> bool {
        for (amount, token) in self.amount.iter().zip(&self.token) {
            if transfer.amount.eq(amount) && transfer.token.eq(token) {
                return true
            }
        }

        false
    }
}

impl ActionCmp<Actions> for NormalizedMint {
    fn is_superior_action(&self, other: &Actions) -> bool {
        match other {
            Actions::Transfer(t) => self.is_superior_action(t),
            _ => false,
        }
    }
}

impl ActionCmp<NormalizedTransfer> for NormalizedCollect {
    fn is_superior_action(&self, transfer: &NormalizedTransfer) -> bool {
        for (amount, token) in self.amount.iter().zip(&self.token) {
            if transfer.amount.eq(amount) && transfer.token.eq(token) {
                return true
            }
        }

        false
    }
}

impl ActionCmp<Actions> for NormalizedCollect {
    fn is_superior_action(&self, other: &Actions) -> bool {
        match other {
            Actions::Transfer(t) => self.is_superior_action(t),
            _ => false,
        }
    }
}
