use brontes_types::{
    normalized_actions::{
        Action, MultiCallFrameClassification, MultiFrameAction, MultiFrameRequest,
    },
    Protocol, TreeSearchBuilder,
};
use tracing::error;

use crate::multi_frame_classification::MultiCallFrameClassifier;

pub struct ZeroXBatch;

impl MultiCallFrameClassifier for ZeroXBatch {
    const KEY: [u8; 2] = [Protocol::ZeroX as u8, MultiFrameAction::Batch as u8];

    fn create_classifier(
        request: MultiFrameRequest,
    ) -> Option<MultiCallFrameClassification<Action>> {
        Some(MultiCallFrameClassification {
            trace_index: request.trace_idx,
            tree_search_builder: TreeSearchBuilder::new().with_actions([
                Box::new(Action::is_swap),
                Box::new(Action::is_transfer),
                Box::new(Action::is_eth_transfer),
            ]),
            parse_fn: Box::new(|this_action, child_nodes| {
                let this = this_action.try_batch_mut().unwrap();
                let mut nodes_to_prune = Vec::new();

                // collect all solver swaps
                let mut solver_swaps = vec![];
                for (trace_index, action) in child_nodes {
                    match &action {
                        Action::Swap(s) => {
                            solver_swaps.push(s.clone());
                            nodes_to_prune.push(trace_index);
                        }
                        Action::SwapWithFee(s) => {
                            solver_swaps.push(s.swap.clone());
                            nodes_to_prune.push(trace_index);
                        }
                        _ => {
                            error!(
                                "Unexpected action in cowswap batch classification: {:?}",
                                action
                            );
                        }
                    }
                }

                this.solver_swaps = Some(solver_swaps);
                nodes_to_prune
            }),
        })
    }
}
