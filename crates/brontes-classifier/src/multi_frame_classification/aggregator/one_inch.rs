use brontes_types::{
    normalized_actions::{
        Actions, MultiCallFrameClassification, MultiFrameAction, MultiFrameRequest, NodeDataIndex,
    },
    Protocol, TreeSearchBuilder,
};

use crate::multi_frame_classification::MultiCallFrameClassifier;

pub struct OneInchAggregator;
pub struct OneInchFusion;

impl MultiCallFrameClassifier for OneInchAggregator {
    const KEY: [u8; 2] = [Protocol::OneInchV5 as u8, MultiFrameAction::Aggregator as u8];

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
            parse_fn:            Box::new(parse_1inch),
        })
    }
}

impl MultiCallFrameClassifier for OneInchFusion {
    const KEY: [u8; 2] = [Protocol::OneInchFusion as u8, MultiFrameAction::Aggregator as u8];

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
            parse_fn:            Box::new(parse_1inch),
        })
    }
}

fn parse_1inch(
    this_action: &mut Actions,
    child_nodes: Vec<(NodeDataIndex, Actions)>,
) -> Vec<NodeDataIndex> {
    let this = this_action.try_aggregator_mut().unwrap();
    let mut prune_nodes = Vec::new();

    for (trace_index, action) in child_nodes {
        match action {
            Actions::Swap(_)
            | Actions::SwapWithFee(_)
            | Actions::Transfer(_)
            | Actions::EthTransfer(_) => {
                this.child_actions.push(action.clone());
                prune_nodes.push(trace_index);
            }
            _ => {}
        }
    }
    prune_nodes
}
