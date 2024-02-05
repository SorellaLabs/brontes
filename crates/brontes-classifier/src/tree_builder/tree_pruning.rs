use brontes_types::{
    normalized_actions::{Actions, NormalizedSwapWithFee},
    tree::BlockTree,
    unzip_either::IterExt,
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
            let swap_data = &node.data.force_swap_ref();
            other_nodes
                .into_iter()
                .filter_map(|(index, data)| {
                    let Actions::Transfer(transfer) = data else { return None };
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
    tree.modify_spans(
        |node| {
            node.get_all_sub_actions().iter().any(|d| d.is_swap())
                && node.get_all_sub_actions().iter().any(|d| d.is_transfer())
        },
        |span| {
            let (swaps, mut transfers): (Vec<_>, Vec<_>) = span
                .into_iter()
                .filter_map(|action| {
                    if action.data.is_swap() {
                        return Some((Some(action), None))
                    } else if action.data.is_transfer() {
                        return Some((None, Some(action)))
                    }
                    None
                })
                .unzip_either();

            for node in swaps {
                transfers.iter_mut().for_each(|transfer| {
                    let mut swap = node.data.clone().force_swap();
                    let transfer = transfer.data.force_transfer_mut();
                    tracing::info!("{:#?}", transfer);

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
                        node.data = swap;
                        tracing::info!("fee on amount out: {:?}", node.data);
                        return
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
                        node.data = swap;
                        tracing::info!("fee on amount in: {:?}", node.data);
                        return
                    }
                });
            }
        },
    );
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
