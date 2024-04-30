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
                        return Some(*index)
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
                            return Some(*index)
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
                            return Some(*index)
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
                            return Some(*index)
                        }
                    }
                    None
                })
                .collect::<Vec<_>>()
        },
    );
}
