use super::{NormalizedCollect, NormalizedMint, NormalizedSwap, NormalizedTransfer};

pub trait ActionCmp<O> {
    /// checks if this action is the superior action. eg Swap is the superior
    /// action to a transfer related to the swap
    fn is_superior_action(&self, other: &O) -> bool;
}

impl ActionCmp<NormalizedTransfer> for NormalizedSwap {
    fn is_superior_action(&self, transfer: &NormalizedTransfer) -> bool {
        (&transfer.amount + &transfer.fee == self.amount_in
            && transfer.to == self.pool
            && self.from == transfer.from)
            || (&transfer.amount - &transfer.fee == self.amount_out
                && transfer.from == self.pool
                && self.recipient == transfer.to)
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
