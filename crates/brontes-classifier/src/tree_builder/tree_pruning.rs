use brontes_types::{
    normalized_actions::{Actions, NormalizedSwapWithFee},
    tree::BlockTree,
};

pub(crate) fn remove_swap_transfers(tree: &mut BlockTree<Actions>) {
    tree.remove_duplicate_data(
        |node| {
            (
                node.data.is_swap(),
                node.get_all_sub_actions()
                    .into_iter()
                    .any(|data| data.is_swap()),
            )
        },
        |node| {
            (
                node.data.is_transfer(),
                node.get_all_sub_actions()
                    .into_iter()
                    .any(|data| data.is_transfer()),
            )
        },
        |node| (node.index, node.data.clone()),
        |other_nodes, node| {
            // calcuate the
            let Actions::Swap(swap_data) = &node.data else { unreachable!() };
            other_nodes
                .into_iter()
                .filter_map(|(index, data)| {
                    let Actions::Transfer(transfer) = data else { return None };
                    if (transfer.amount == swap_data.amount_in
                        || transfer.amount == swap_data.amount_out)
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
pub(crate) fn remove_mint_transfers(tree: &mut BlockTree<Actions>) {
    tree.remove_duplicate_data(
        |node| {
            (
                node.data.is_mint(),
                node.get_all_sub_actions()
                    .into_iter()
                    .any(|data| data.is_mint()),
            )
        },
        |node| {
            (
                node.data.is_transfer(),
                node.get_all_sub_actions()
                    .into_iter()
                    .any(|data| data.is_transfer()),
            )
        },
        |node| (node.index, node.data.clone()),
        |other_nodes, node| {
            let Actions::Mint(mint_data) = &node.data else { unreachable!() };
            other_nodes
                .into_iter()
                .filter_map(|(index, data)| {
                    let Actions::Transfer(transfer) = data else { return None };
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

pub(crate) fn remove_collect_transfers(tree: &mut BlockTree<Actions>) {
    tree.remove_duplicate_data(
        |node| {
            (
                node.data.is_collect(),
                node.get_all_sub_actions()
                    .into_iter()
                    .any(|data| data.is_collect()),
            )
        },
        |node| {
            (
                node.data.is_transfer(),
                node.get_all_sub_actions()
                    .into_iter()
                    .any(|data| data.is_transfer()),
            )
        },
        |node| (node.index, node.data.clone()),
        |other_nodes, node| {
            let Actions::Collect(collect_data) = &node.data else { unreachable!() };
            other_nodes
                .into_iter()
                .filter_map(|(index, data)| {
                    let Actions::Transfer(transfer) = data else { return None };
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

/// When a tax token takes a fee, They will swap from there token to a more
/// stable token like eth before taking the fee. However this creates an
/// accounting inaccuracy as we will register this fee swap as
/// part of the mev messing up our profit accounting.
pub(crate) fn account_for_tax_tokens(tree: &mut BlockTree<Actions>) {
    // remove swaps that originate from a transfer. This event only occurs
    // when a tax token is transfered and the taxed amount is swapped into
    // a more stable currency
    tree.modify_node_if_contains_childs(
        |node| {
            let mut has_transfer = false;
            let mut has_swap = false;

            for action in &node.get_all_sub_actions() {
                if action.is_transfer() {
                    has_transfer = true;
                } else if action.is_swap() {
                    has_swap = true;
                }
            }
            (node.data.is_transfer(), has_swap && has_transfer)
        },
        |node| {
            let mut swap_idx = Vec::new();
            node.collect(
                &mut swap_idx,
                &|node| {
                    (
                        node.data.is_swap(),
                        node.get_all_sub_actions()
                            .iter()
                            .any(|action| action.is_swap()),
                    )
                },
                &|node| node.index,
            );

            swap_idx.into_iter().for_each(|idx| {
                node.remove_node_and_children(idx);
            })
        },
    );

    // adjusts the amount in of the swap and notes the fee on the normalized type.
    // This is needed when swapping into the tax token as the amount out of the swap
    // will be wrong
    tree.modify_node_if_contains_childs(
        |node| {
            let mut has_transfer = false;
            let mut has_swap = false;
            for action in &node.get_all_sub_actions() {
                if action.is_transfer() {
                    has_transfer = true;
                } else if action.is_swap() {
                    has_swap = true;
                }
            }
            (node.data.is_swap(), has_swap && has_transfer)
        },
        |node| {
            // collect all sub transfers
            let mut transfers = Vec::new();
            node.collect(
                &mut transfers,
                &|node| {
                    (
                        node.data.is_transfer(),
                        node.get_all_sub_actions()
                            .iter()
                            .any(|node| node.is_transfer()),
                    )
                },
                &|node| node.data.clone(),
            );

            transfers
                .into_iter()
                .filter_map(
                    |transfer| {
                        if let Actions::Transfer(t) = transfer {
                            Some(t)
                        } else {
                            None
                        }
                    },
                )
                .for_each(|transfer| {
                    tracing::info!(?transfer);
                    let mut swap = node.data.clone().force_swap();
                    // adjust the amount out case
                    if swap.token_out == transfer.token
                        && swap.pool == transfer.from
                        && swap.recipient == transfer.token.address
                        && swap.amount_out > transfer.amount
                    {
                        let fee_amount = swap.amount_out - &transfer.amount;
                        swap.amount_out = transfer.amount;

                        let swap = Actions::SwapWithFee(NormalizedSwapWithFee {
                            swap,
                            fee_amount,
                            fee_token: transfer.token,
                        });
                        node.data = swap;
                        return
                    }
                    // adjust the amount in case
                    else if swap.token_in == transfer.token
                        && swap.pool == transfer.to
                        && swap.amount_in != transfer.amount
                    {
                        let fee_amount = transfer.amount.clone();
                        swap.amount_in += transfer.amount;
                        let swap = Actions::SwapWithFee(NormalizedSwapWithFee {
                            swap,
                            fee_amount,
                            fee_token: transfer.token,
                        });
                        node.data = swap;
                        return
                    }
                });
        },
    )
}

#[cfg(test)]
mod test {
    use hex_literal::hex;

    use crate::test_utils::ClassifierTestUtils;

    /// 7 total swaps but 1 is tax token
    #[tokio::test]
    #[serial_test::serial]
    async fn test_filter_tax_tokens() {
        let mut utils = ClassifierTestUtils::new();
        let tree = utils
            .build_tree_tx(
                hex!("8ea5ea6de313e466483f863071461992b3ea3278e037513b0ad9b6a29a4429c1").into(),
            )
            .await
            .unwrap();

        let swaps = tree.collect(
            hex!("8ea5ea6de313e466483f863071461992b3ea3278e037513b0ad9b6a29a4429c1").into(),
            |node| (node.data.is_swap(), node.inner.iter().any(|n| n.data.is_swap())),
        );
        assert!(swaps.len() == 6, "didn't filter tax token");
    }
}
