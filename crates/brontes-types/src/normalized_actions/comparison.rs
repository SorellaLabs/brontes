use std::fmt::Debug;

use super::{Action, NormalizedCollect, NormalizedMint, NormalizedSwap, NormalizedTransfer};

impl<T: Sized + SubordinateAction<O>, O: ActionCmp<T>> ActionComparison<O> for T {}

pub trait ActionComparison<O> {
    fn is_same_coverage(&self, other: &O) -> bool
    where
        Self: Sized + ActionCmp<O> + Debug,
        O: ActionCmp<Self> + Debug,
    {
        self.is_superior_action(other) || self.is_subordinate(other)
    }
}

impl ActionCmp<Action> for Action {
    fn is_superior_action(&self, other: &Action) -> bool {
        match self {
            Action::Swap(s) => s.is_superior_action(other),
            Action::Mint(m) => m.is_superior_action(other),
            Action::Collect(c) => c.is_superior_action(other),
            Action::SwapWithFee(s) => s.swap.is_superior_action(other),
            Action::FlashLoan(f) => f.child_actions.iter().any(|a| a.is_superior_action(other)),
            Action::Batch(b) => {
                let user = b.user_swaps.iter().any(|b| b.is_superior_action(other));
                if let Some(swaps) = &b.solver_swaps {
                    return user || swaps.iter().any(|b| b.is_superior_action(other))
                }
                user
            }
            _ => false,
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
        // we cannot filter on from address for a transfer to a pool.
        // this is because you can have a pool transfer the token to another pool, but
        // your contract has to call it
        (&transfer.amount + &transfer.fee == self.amount_in && transfer.to == self.pool)
            || (transfer.amount == self.amount_out
                && transfer.from == self.pool
                && self.recipient == transfer.to)
    }
}

impl ActionCmp<Action> for NormalizedSwap {
    fn is_superior_action(&self, other: &Action) -> bool {
        match other {
            Action::Transfer(t) => self.is_superior_action(t),
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

impl ActionCmp<Action> for NormalizedMint {
    fn is_superior_action(&self, other: &Action) -> bool {
        match other {
            Action::Transfer(t) => self.is_superior_action(t),
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

impl ActionCmp<Action> for NormalizedCollect {
    fn is_superior_action(&self, other: &Action) -> bool {
        match other {
            Action::Transfer(t) => self.is_superior_action(t),
            _ => false,
        }
    }
}
