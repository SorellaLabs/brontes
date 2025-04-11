use brontes_types::TreeSearchFn;
use brontes_types::{
    normalized_actions::{
        Action, MultiCallFrameClassification, MultiFrameAction, MultiFrameRequest,
    },
    Protocol, TreeSearchBuilder,
};

use crate::multi_frame_classification::MultiCallFrameClassifier;

pub struct ZeroXAgg;

impl MultiCallFrameClassifier for ZeroXAgg {
    const KEY: [u8; 2] = [Protocol::ZeroX as u8, MultiFrameAction::Aggregator as u8];

    fn create_classifier(
        request: MultiFrameRequest,
    ) -> Option<MultiCallFrameClassification<Action>> {
        Some(MultiCallFrameClassification {
            trace_index: request.trace_idx,
            tree_search_builder: TreeSearchBuilder::new().with_actions([
                Action::is_swap.boxed(),
                Action::is_transfer.boxed(),
                Action::is_eth_transfer.boxed(),
            ]),
            parse_fn: Box::new(|this_action, child_nodes| {
                let this = this_action.try_aggregator_mut().unwrap();
                let mut prune_nodes = Vec::new();

                for (trace_index, action) in child_nodes {
                    match action {
                        Action::Swap(_)
                        | Action::SwapWithFee(_)
                        | Action::Transfer(_)
                        | Action::EthTransfer(_) => {
                            this.child_actions.push(action.clone());
                            prune_nodes.push(trace_index);
                        }
                        _ => {}
                    }
                }
                prune_nodes
            }),
        })
    }
}
