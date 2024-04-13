use brontes_types::{
    db::token_info::TokenInfoWithAddress,
    normalized_actions::{
        Actions, MultiCallFrameClassification, MultiFrameAction, MultiFrameRequest,
    },
    Protocol, ToScaledRational, TreeSearchBuilder,
};
use tracing::error;

use super::MultiCallFrameClassifier;

pub struct UniswapX;

impl MultiCallFrameClassifier for UniswapX {
    const KEY: [u8; 2] = [Protocol::UniswapX as u8, MultiFrameAction::Batch as u8];

    fn create_classifier(
        request: MultiFrameRequest,
    ) -> Option<MultiCallFrameClassification<Actions>> {
        Some(MultiCallFrameClassification {
            trace_index:         request.trace_idx,
            tree_search_builder: TreeSearchBuilder::new().with_actions([
                Actions::is_swap,
                Actions::is_transfer,
                Actions::is_eth_transfer,
            ]),
            parse_fn:            Box::new(|this_action, child_nodes| {
                let this = this_action.try_batch_mut().unwrap();
                let mut nodes_to_prune = Vec::new();

                for (trace_index, action) in child_nodes {
                    match &action {
                        Actions::Transfer(t) => {
                            for user_swap in &mut this.user_swaps {
                                if t.from == user_swap.from && t.to == this.solver {
                                    user_swap.trace_index = trace_index.trace_index;
                                    user_swap.token_in = t.token.clone();
                                    user_swap.amount_in = t.amount.clone();
                                    break
                                } else if t.from == this.solver && t.to == user_swap.from {
                                    user_swap.token_out = t.token.clone();
                                    user_swap.amount_out = t.amount.clone();
                                    break
                                }
                            }
                        }
                        Actions::EthTransfer(et) => {
                            for user_swap in &mut this.user_swaps {
                                if et.from == user_swap.from && et.to == this.settlement_contract {
                                    user_swap.trace_index = trace_index.trace_index;
                                    user_swap.token_in = TokenInfoWithAddress::native_eth();
                                    user_swap.amount_in = et.clone().value.to_scaled_rational(18);
                                    break
                                } else if et.from == this.settlement_contract
                                    && et.to == user_swap.from
                                {
                                    user_swap.token_out = TokenInfoWithAddress::native_eth();
                                    user_swap.amount_out = et.clone().value.to_scaled_rational(18);
                                    break
                                }
                            }
                        }
                        Actions::Swap(s) => {
                            if let Some(swaps) = &mut this.solver_swaps {
                                swaps.push(s.clone());
                                nodes_to_prune.push(trace_index);
                                break
                            } else {
                                this.solver_swaps = Some(vec![s.clone()]);
                                nodes_to_prune.push(trace_index);
                                break
                            }
                        }
                        Actions::SwapWithFee(s) => {
                            if let Some(swaps) = &mut this.solver_swaps {
                                swaps.push(s.swap.clone());
                                nodes_to_prune.push(trace_index);
                                break
                            } else {
                                this.solver_swaps = Some(vec![s.swap.clone()]);
                                nodes_to_prune.push(trace_index);
                                break
                            }
                        }
                        _ => {
                            error!(
                                "Unexpected action in uniswap x batch classification: {:?}",
                                action
                            );
                        }
                    }
                }
                nodes_to_prune
            }),
        })
    }
}
