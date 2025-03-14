use crate::{normalized_actions::Action, tree::BlockTree, TreeSearchBuilder};

pub fn remove_swap_transfers(tree: &mut BlockTree<Action>) {
    tree.remove_duplicate_data(
        TreeSearchBuilder::default().with_action(Action::is_swap),
        TreeSearchBuilder::default().with_action(Action::is_transfer),
        |data| (data.node.index, data.data.clone()),
        |other_nodes, node, data| {
            // calcuate the
            let Some(swap_data) = data.get_ref(node.data).and_then(|node| node.first()) else {
                return vec![];
            };
            let swap_data = swap_data.force_swap_ref();

            other_nodes
                .iter()
                .filter_map(|(index, data)| {
                    let Action::Transfer(transfer) = data else {
                        return None;
                    };
                    if (transfer.amount == swap_data.amount_in
                        || (&transfer.amount + &transfer.fee) == swap_data.amount_out)
                        && (transfer.to == swap_data.pool || transfer.from == swap_data.pool)
                    {
                        return Some(*index);
                    }
                    None
                })
                .collect::<Vec<_>>()
        },
    );
}
pub fn remove_mint_transfers(tree: &mut BlockTree<Action>) {
    tree.remove_duplicate_data(
        TreeSearchBuilder::default().with_action(Action::is_mint),
        TreeSearchBuilder::default().with_action(Action::is_transfer),
        |data| (data.node.index, data.data.clone()),
        |other_nodes, node, node_data| {
            let Some(Action::Mint(mint_data)) =
                node_data.get_ref(node.data).and_then(|node| node.first())
            else {
                unreachable!("value not mint")
            };
            other_nodes
                .iter()
                .filter_map(|(index, data)| {
                    let Action::Transfer(transfer) = data else {
                        return None;
                    };
                    for (amount, token) in mint_data.amount.iter().zip(&mint_data.token) {
                        if transfer.amount.eq(amount) && transfer.token.eq(token) {
                            return Some(*index);
                        }
                    }
                    None
                })
                .collect::<Vec<_>>()
        },
    );
}

pub fn remove_burn_transfers(tree: &mut BlockTree<Action>) {
    tree.remove_duplicate_data(
        TreeSearchBuilder::default().with_action(Action::is_burn),
        TreeSearchBuilder::default().with_action(Action::is_transfer),
        |data| (data.node.index, data.data.clone()),
        |other_nodes, node, node_data| {
            let Some(Action::Burn(burn_data)) =
                node_data.get_ref(node.data).and_then(|node| node.first())
            else {
                unreachable!("value not burn")
            };
            other_nodes
                .iter()
                .filter_map(|(index, data)| {
                    let Action::Transfer(transfer) = data else {
                        return None;
                    };
                    for (amount, token) in burn_data.amount.iter().zip(&burn_data.token) {
                        if transfer.amount.eq(amount) && transfer.token.eq(token) {
                            return Some(*index);
                        }
                    }
                    None
                })
                .collect::<Vec<_>>()
        },
    );
}

pub fn remove_collect_transfers(tree: &mut BlockTree<Action>) {
    tree.remove_duplicate_data(
        TreeSearchBuilder::default().with_action(Action::is_collect),
        TreeSearchBuilder::default().with_action(Action::is_transfer),
        |data| (data.node.index, data.data.clone()),
        |other_nodes, node, node_info| {
            let Some(Action::Collect(collect_data)) =
                node_info.get_ref(node.data).and_then(|node| node.first())
            else {
                unreachable!("value not collect")
            };
            other_nodes
                .iter()
                .filter_map(|(index, data)| {
                    let Action::Transfer(transfer) = data else {
                        return None;
                    };
                    for (amount, token) in collect_data.amount.iter().zip(&collect_data.token) {
                        if transfer.amount.eq(amount) && transfer.token.eq(token) {
                            return Some(*index);
                        }
                    }
                    None
                })
                .collect::<Vec<_>>()
        },
    );
}

#[cfg(test)]
pub mod test {

    use std::sync::Arc;

    use alloy_primitives::hex;
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        normalized_actions::Action,
        tree::{remove_swap_transfers, BlockTree},
        TreeSearchBuilder,
    };

    /// There was a problem with the tree not showing all orders for the
    /// de-duplication that would cause some to be missed. If it is fixed for
    /// swaps, will be fixed for the rest
    #[brontes_macros::test]
    pub async fn test_transfer_de_duplication() {
        let tx_hash =
            hex!("07a0580a713928d78caad0d09b20d23ae6fba47753b8007e6313f911fc9084be").into();

        let utils = ClassifierTestUtils::new().await;
        let mut tree: BlockTree<Action> = utils.build_tree_tx(tx_hash).await.unwrap();
        let search_args = TreeSearchBuilder::default().with_action(Action::is_transfer);
        let transfers: Vec<Action> = Arc::new(tree.clone())
            .collect(&tx_hash, search_args.clone())
            .collect::<Vec<_>>();
        assert_eq!(transfers.len(), 4);

        // dedup tree
        remove_swap_transfers(&mut tree);
        let transfers: Vec<Action> = Arc::new(tree.clone())
            .collect(&tx_hash, search_args)
            .collect::<Vec<_>>();
        assert_eq!(transfers.len(), 0);
    }
}
