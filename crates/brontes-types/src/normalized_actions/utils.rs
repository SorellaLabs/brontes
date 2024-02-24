use super::{NormalizedCollect, NormalizedMint, NormalizedSwap, NormalizedTransfer};

pub trait ActionCmp<O> {
    /// checks if this action is the superior action. eg Swap is the superior
    /// action to a transfer related to the swap
    fn is_superior_action(&self, other: &O) -> bool;
}

impl ActionCmp<NormalizedTransfer> for NormalizedSwap {
    fn is_superior_action(&self, transfer: &NormalizedTransfer) -> bool {
        (transfer.amount == self.amount_in || (&transfer.amount + &transfer.fee) == self.amount_out)
            && (transfer.to == self.pool || transfer.from == self.pool)
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
