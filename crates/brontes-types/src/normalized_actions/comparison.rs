use super::{Actions, NormalizedCollect, NormalizedMint, NormalizedSwap, NormalizedTransfer};

impl<T: Sized + SubordinateAction<O>, O: ActionCmp<T>> ActionComparison<O> for T {}

pub trait ActionComparison<O> {
    fn is_same_coverage(&self, other: &O) -> bool
    where
        Self: Sized + ActionCmp<O>,
        O: ActionCmp<Self>,
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
                tracing::debug!(?action, "no action cmp impl for given action");
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
        Self: Sized,
        O: ActionCmp<Self>,
    {
        other.is_superior_action(self)
    }
}

impl ActionCmp<NormalizedTransfer> for NormalizedSwap {
    fn is_superior_action(&self, transfer: &NormalizedTransfer) -> bool {
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

#[cfg(test)]
pub mod test {
    use alloy_primitives::hex;
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        normalized_actions::Actions, ActionIter, BlockTree, TreeCollect, TreeFilter,
        TreeSearchBuilder,
    };

    #[brontes_macros::test]
    async fn test_swap_transfer_dedup() {
        let utils = ClassifierTestUtils::new().await;
        let tx = hex!("c6b9e1c5e5478defaae7b2cdb8aeaf22cc16bec599cb5fdad470429919dd8f70").into();
        let tree: BlockTree<Actions> = utils.build_tree_tx(tx).await.unwrap();

        let call =
            TreeSearchBuilder::default().with_actions([Actions::is_transfer, Actions::is_swap]);
        let default_collect = tree.collect(&tx, call.clone());

        let (swaps, transfers): (Vec<_>, Vec<_>) = default_collect
            .into_iter()
            .action_split((Actions::try_swaps_merged, Actions::try_transfer));

        assert_eq!(swaps.len(), 1, "action split broken swaps {:#?}", swaps);
        assert_eq!(transfers.len(), 2, "action split broken transfer {:#?}", transfers);

        let (transfers, swaps): (Vec<_>, Vec<_>) = tree.collect_actions_filter(
            &tx,
            call.clone(),
            (Actions::try_transfer, Actions::try_swaps_merged),
        );
        assert_eq!(swaps.len(), 1, "no swap found {:#?}", swaps);
        assert_eq!(transfers.len(), 2, "missing transfers {:#?}", transfers);

        let deduped_transfers = tree
            .collect_tx_deduping(
                &tx,
                call.clone(),
                (Actions::try_swaps_merged_dedup(),),
                (Actions::try_transfer_dedup(),),
            )
            .into_iter()
            .collect_action_vec(Actions::try_transfer);

        assert_eq!(deduped_transfers.len(), 0, "{:#?}", deduped_transfers);
    }
}
