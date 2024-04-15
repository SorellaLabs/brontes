use brontes_types::{
    normalized_actions::{Actions, NormalizedSwapWithFee},
    tree::BlockTree,
    unzip_either::IterExt,
    TreeCollector, TreeSearchBuilder,
};
use malachite::{num::basic::traits::Zero, Rational};

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
                .filter_map(|action| Some((action.data, data.get_ref(action.data)?)))
                .filter_map(|(idx, data)| {
                    let (mut swaps, mut transfers, mut eth_transfers): (Vec<_>, Vec<_>, Vec<_>) =
                        data.clone().into_iter().split_actions((
                            Actions::try_swap,
                            Actions::try_transfer,
                            Actions::try_eth_transfer,
                        ));

                    if !swaps.is_empty() {
                        return Some((
                            Some(((swaps.pop().unwrap(), eth_transfers.pop()), idx)),
                            None,
                        ))
                    } else if !transfers.is_empty() {
                        return Some((
                            None,
                            Some(((transfers.pop().unwrap(), eth_transfers.pop()), idx)),
                        ))
                    }
                    None
                })
                .unzip_either();

            for ((mut swap, eth_transfer), swap_idx) in swaps {
                transfers.iter_mut().for_each(|((transfer, _), _)| {
                    if transfer.fee == Rational::ZERO {
                        return
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

                        let mut swap = vec![Actions::SwapWithFee(NormalizedSwapWithFee {
                            swap: swap.clone(),
                            fee_amount,
                            fee_token: transfer.token.clone(),
                        })];

                        if let Some(eth_t) = eth_transfer.clone() {
                            swap.push(Actions::EthTransfer(eth_t));
                        }

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
                        let mut swap = vec![Actions::SwapWithFee(NormalizedSwapWithFee {
                            swap: swap.clone(),
                            fee_amount,
                            fee_token: transfer.token.clone(),
                        })];
                        if let Some(eth_t) = eth_transfer.clone() {
                            swap.push(Actions::EthTransfer(eth_t));
                        }
                        data.replace(swap_idx, swap);
                        return
                    }
                });
            }
        },
    );
    // remove swaps that originate from a transfer. This event only occurs
    // when a tax token is transfered and the taxed amount is swapped into
    // a more stable currency
    // tree.modify_node_if_contains_childs(
    //     TreeSearchBuilder::default()
    //         .with_action(Actions::is_transfer)
    //         .child_nodes_contain([Actions::is_swap, Actions::is_transfer]),
    //     |node, data| {
    //         let mut swap_idx = Vec::new();
    //         node.collect(
    //             &mut swap_idx,
    //             &TreeSearchBuilder::default().with_action(Actions::is_swap),
    //             &|node| node.node.index,
    //             data,
    //         );
    //
    //         swap_idx.into_iter().for_each(|idx| {
    //             node.remove_node_and_children(idx, data);
    //         })
    //     },
    // );
}

pub(crate) fn remove_possible_transfer_double_counts(tree: &mut BlockTree<Actions>) {
    tracing::debug!("remove double transfer counts");
    tree.modify_node_if_contains_childs(
        TreeSearchBuilder::default().with_action(Actions::is_transfer),
        |node, data| {
            let mut inner_transfers = Vec::new();
            node.collect(
                &mut inner_transfers,
                &TreeSearchBuilder::default().with_action(Actions::is_transfer),
                &|node| node.node.clone(),
                data,
            );

            let this = data
                .get_ref(node.data)
                .unwrap()
                .first()
                .unwrap()
                .clone()
                .force_transfer();

            inner_transfers.into_iter().for_each(|i_transfer| {
                if let Some(i_data) = data.get_mut(i_transfer.data) {
                    let f = i_data.get_mut(0).unwrap();
                    if let Actions::Transfer(t) = f {
                        if this.to == t.to
                            && this.from == t.from
                            && this.amount == t.amount
                            && this.token == t.token
                            && this.trace_index != t.trace_index
                        {
                            tracing::debug!(?t, ?this, "setting amount to zero");
                            t.amount = Rational::ZERO;
                        }
                    }
                }
            });
        },
    );
}
