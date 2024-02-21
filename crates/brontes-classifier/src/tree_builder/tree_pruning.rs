use brontes_types::{
    normalized_actions::{Actions, NormalizedSwapWithFee},
    tree::BlockTree,
    unzip_either::IterExt,
    TreeSearchBuilder,
};
use malachite::{num::basic::traits::Zero, Rational};

// pub(crate) fn remove_swap_transfers(tree: &mut BlockTree<Actions>) {
//     tree.remove_duplicate_data(
//         |node, data| TreeSearchArgs {
//             collect_current_node: data
//                 .get_ref(node.data)
//                 .map(|data| data.is_swap())
//                 .unwrap_or_default(),
//             child_node_to_collect: node
//                 .get_all_sub_actions()
//                 .into_iter()
//                 .filter_map(|a| data.get_ref(a))
//                 .any(|data| data.is_swap()),
//         },
//         |node, data| TreeSearchArgs {
//             collect_current_node: data
//                 .get_ref(node.data)
//                 .map(|data| data.is_transfer())
//                 .unwrap_or_default(),
//             child_node_to_collect: node
//                 .get_all_sub_actions()
//                 .into_iter()
//                 .filter_map(|a| data.get_ref(a))
//                 .any(|data| data.is_transfer()),
//         },
//         |node, data| (node.index, data.get_ref(node.data).cloned()),
//         |other_nodes, node, data| {
//             // calcuate the
//             let Some(swap_data) = data.get_ref(node.data) else {
//                 return vec![];
//             };
//             let swap_data = swap_data.force_swap_ref();
//
//             other_nodes
//                 .iter()
//                 .filter_map(|(index, data)| {
//                     let Actions::Transfer(transfer) = data.as_ref()? else {
//                         return None;
//                     };
//                     if (transfer.amount == swap_data.amount_in
//                         || (&transfer.amount + &transfer.fee) == swap_data.amount_out)
//                         && (transfer.to == swap_data.pool || transfer.from ==
// swap_data.pool)                     {
//                         return Some(*index);
//                     }
//                     None
//                 })
//                 .collect::<Vec<_>>()
//         },
//     );
// }
// pub(crate) fn remove_mint_transfers(tree: &mut BlockTree<Actions>) {
//     tree.remove_duplicate_data(
//         |node, data| TreeSearchArgs {
//             collect_current_node: data
//                 .get_ref(node.data)
//                 .map(|data| data.is_mint())
//                 .unwrap_or_default(),
//             child_node_to_collect: node
//                 .get_all_sub_actions()
//                 .into_iter()
//                 .filter_map(|a| data.get_ref(a))
//                 .any(|data| data.is_mint()),
//         },
//         |node, data| TreeSearchArgs {
//             collect_current_node: data
//                 .get_ref(node.data)
//                 .map(|data| data.is_transfer())
//                 .unwrap_or_default(),
//             child_node_to_collect: node
//                 .get_all_sub_actions()
//                 .into_iter()
//                 .filter_map(|a| data.get_ref(a))
//                 .any(|data| data.is_transfer()),
//         },
//         |node, data| (node.index, data.get_ref(node.data).cloned()),
//         |other_nodes, node, node_data| {
//             let Some(Actions::Mint(mint_data)) = node_data.get_ref(node.data)
// else {                 unreachable!("value not mint")
//             };
//             other_nodes
//                 .iter()
//                 .filter_map(|(index, data)| {
//                     let Actions::Transfer(transfer) = data.as_ref()? else {
//                         return None;
//                     };
//                     for (amount, token) in
// mint_data.amount.iter().zip(&mint_data.token) {                         if
// transfer.amount.eq(amount) && transfer.token.eq(token) {
// return Some(*index);                         }
//                     }
//                     None
//                 })
//                 .collect::<Vec<_>>()
//         },
//     );
// }
//
// pub(crate) fn remove_collect_transfers(tree: &mut BlockTree<Actions>) {
//     tree.remove_duplicate_data(
//         |node, data| TreeSearchArgs {
//             collect_current_node: data
//                 .get_ref(node.data)
//                 .map(|data| data.is_collect())
//                 .unwrap_or_default(),
//             child_node_to_collect: node
//                 .get_all_sub_actions()
//                 .into_iter()
//                 .filter_map(|a| data.get_ref(a))
//                 .any(|data| data.is_collect()),
//         },
//         |node, data| TreeSearchArgs {
//             collect_current_node: data
//                 .get_ref(node.data)
//                 .map(|data| data.is_transfer())
//                 .unwrap_or_default(),
//             child_node_to_collect: node
//                 .get_all_sub_actions()
//                 .into_iter()
//                 .filter_map(|a| data.get_ref(a))
//                 .any(|data| data.is_transfer()),
//         },
//         |node, data| (node.index, data.get_ref(node.data).cloned()),
//         |other_nodes, node, node_info| {
//             let Some(Actions::Collect(collect_data)) =
// node_info.get_ref(node.data) else {                 unreachable!("value not
// collect")             };
//             other_nodes
//                 .iter()
//                 .filter_map(|(index, data)| {
//                     let Actions::Transfer(transfer) = data.as_ref()? else {
//                         return None;
//                     };
//                     for (amount, token) in
// collect_data.amount.iter().zip(&collect_data.token) {
// if transfer.amount.eq(amount) && transfer.token.eq(token) {
// return Some(*index);                         }
//                     }
//                     None
//                 })
//                 .collect::<Vec<_>>()
//         },
//     );
// }

/// When a tax token takes a fee, They will swap from there token to a more
/// stable token like eth before taking the fee. However this creates an
/// accounting inaccuracy as we will register this fee swap as
/// part of the mev messing up our profit accounting.
pub(crate) fn account_for_tax_tokens(tree: &mut BlockTree<Actions>) {
    // adjusts the amount in of the swap and notes the fee on the normalized type.
    // This is needed when swapping into the tax token as the amount out of the swap
    // will be wrong
    tree.modify_spans(
        TreeSearchBuilder::default()
            .with_action(Actions::is_swap)
            .child_nodes_have([Actions::is_transfer]),
        |span, data| {
            let (swaps, mut transfers): (Vec<_>, Vec<_>) = span
                .into_iter()
                .filter_map(|action| {
                    let data = data.get_ref(action.data)?;
                    if data.is_swap() {
                        return Some((Some((action.data, data.clone())), None));
                    } else if data.is_transfer() {
                        return Some((None, Some((action.data, data.clone()))));
                    }
                    None
                })
                .unzip_either();

            for (swap_idx, node) in swaps {
                transfers.iter_mut().for_each(|(_, transfer)| {
                    let mut swap = node.clone().force_swap();
                    let transfer = transfer.force_transfer_mut();
                    if transfer.fee == Rational::ZERO {
                        return;
                    }

                    // adjust the amount out case
                    if swap.token_out == transfer.token
                        && swap.pool == transfer.from
                        && swap.recipient == transfer.to
                        && swap.amount_out != transfer.amount
                    {
                        let fee_amount = transfer.fee.clone();
                        // token is going out so the amount out on the swap
                        // will be with fee.
                        swap.amount_out -= &transfer.fee;

                        let swap = Actions::SwapWithFee(NormalizedSwapWithFee {
                            swap,
                            fee_amount,
                            fee_token: transfer.token.clone(),
                        });
                        data.replace(swap_idx, swap);
                    }
                    // adjust the amount in case
                    else if swap.token_in == transfer.token
                        && swap.pool == transfer.to
                        && swap.amount_in != (&transfer.amount + &transfer.fee)
                    {
                        let fee_amount = transfer.fee.clone();
                        // swap amount in will be the amount without fee.
                        swap.amount_in += &transfer.fee;
                        let swap = Actions::SwapWithFee(NormalizedSwapWithFee {
                            swap,
                            fee_amount,
                            fee_token: transfer.token.clone(),
                        });
                        data.replace(swap_idx, swap);
                        return;
                    }
                });
            }
        },
    );
    // remove swaps that originate from a transfer. This event only occurs
    // when a tax token is transfered and the taxed amount is swapped into
    // a more stable currency
    tree.modify_node_if_contains_childs(
        TreeSearchBuilder::default()
            .with_action(Actions::is_transfer)
            .child_nodes_contain([Actions::is_swap, Actions::is_transfer]),
        |node, data| {
            let mut swap_idx = Vec::new();
            node.collect(
                &mut swap_idx,
                &TreeSearchBuilder::default().with_action(Actions::is_swap),
                &|node, _| node.index,
                data,
            );

            swap_idx.into_iter().for_each(|idx| {
                node.remove_node_and_children(idx, data);
            })
        },
    );
}

#[cfg(test)]
mod test {
    use brontes_types::{normalized_actions::Actions, TreeSearchBuilder};
    use hex_literal::hex;

    use crate::test_utils::ClassifierTestUtils;

    /// 7 total swaps but 1 is tax token
    #[brontes_macros::test]
    async fn test_filter_tax_tokens() {
        let utils = ClassifierTestUtils::new().await;
        let tree = utils
            .build_tree_tx(
                hex!("8ea5ea6de313e466483f863071461992b3ea3278e037513b0ad9b6a29a4429c1").into(),
            )
            .await
            .unwrap();

        let swaps = tree.collect(
            hex!("8ea5ea6de313e466483f863071461992b3ea3278e037513b0ad9b6a29a4429c1").into(),
            TreeSearchBuilder::default().with_action(Actions::is_swap),
        );
        assert!(swaps.len() == 6, "didn't filter tax token");
    }
}
