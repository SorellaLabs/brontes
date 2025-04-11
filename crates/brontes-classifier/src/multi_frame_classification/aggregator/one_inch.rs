use alloy_primitives::{hex, Address};
use brontes_types::{
    normalized_actions::{
        Action, MultiCallFrameClassification, MultiFrameAction, MultiFrameRequest, NodeDataIndex,
    },
    Protocol, TreeSearchBuilder, TreeSearchFn,
};

use crate::multi_frame_classification::MultiCallFrameClassifier;

pub struct OneInchAggregator;
pub struct OneInchFusion;

impl MultiCallFrameClassifier for OneInchAggregator {
    const KEY: [u8; 2] = [Protocol::OneInchV5 as u8, MultiFrameAction::Aggregator as u8];

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
                parse_1inch(this_action, child_nodes, false)
            }),
        })
    }
}

const FUSION_ADDRESS: Address = Address::new(hex!("A88800CD213dA5Ae406ce248380802BD53b47647"));

impl MultiCallFrameClassifier for OneInchFusion {
    const KEY: [u8; 2] = [Protocol::OneInchFusion as u8, MultiFrameAction::Aggregator as u8];

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
                parse_1inch(this_action, child_nodes, true)
            }),
        })
    }
}

fn parse_1inch(
    this_action: &mut Action,
    child_nodes: Vec<(NodeDataIndex, Action)>,
    is_fusion: bool,
) -> Vec<NodeDataIndex> {
    let this = this_action.try_aggregator_mut().unwrap();
    let mut prune_nodes = Vec::new();

    for (trace_index, action) in child_nodes {
        match action {
            Action::Swap(_) | Action::SwapWithFee(_) => {
                this.child_actions.push(action.clone());
                prune_nodes.push(trace_index);
            }
            Action::Transfer(_) | Action::EthTransfer(_) if !is_fusion => {
                this.child_actions.push(action.clone());
                prune_nodes.push(trace_index);
            }
            Action::Transfer(t) if is_fusion => {
                if t.from == FUSION_ADDRESS {
                    this.recipient = t.to;
                }
                this.child_actions.push(t.into());
                prune_nodes.push(trace_index);
            }
            Action::EthTransfer(e) if is_fusion => {
                if e.from == FUSION_ADDRESS {
                    this.recipient = e.to;
                }
                this.child_actions.push(e.into());
                prune_nodes.push(trace_index);
            }

            _ => {}
        }
    }
    prune_nodes
}
