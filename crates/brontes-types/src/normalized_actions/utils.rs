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
            || (transfer.amount == self.amount_out
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

    use crate::{
        normalized_actions::{transfer, Actions},
        ActionIter, BlockTree, TreeCollect, TreeFilter, TreeSearchBuilder,
    };

    #[brontes_macros::test]
    async fn test_swap_transfer_dedup() {
        let utils = ClassifierTestUtils::new().await;
        let tx = hex!("c6b9e1c5e5478defaae7b2cdb8aeaf22cc16bec599cb5fdad470429919dd8f70").into();
        let tree: BlockTree<Actions> = utils.build_tree_tx(tx).await.unwrap();
        let call = TreeSearchBuilder::default().with_action(Actions::is_transfer);

        let transfers = tree.collect_action_filter(tx, call.clone(), Actions::try_transfer);

        assert_eq!(transfers.len(), 3, "{:#?}", transfers);

        let deduped_transfers = tree
            .collect_tx_deduping(
                &tx,
                call.clone(),
                (Actions::try_swap_dedup(),),
                (Actions::try_transfer_dedup(),),
            )
            .into_iter()
            .collect_action_vec(Actions::try_transfer);

        assert_eq!(deduped_transfers.len(), 1, "{:#?}", deduped_transfers);
    }
}
