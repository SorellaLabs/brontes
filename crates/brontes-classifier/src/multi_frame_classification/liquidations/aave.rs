use brontes_types::{
    normalized_actions::{
        Action, MultiCallFrameClassification, MultiFrameAction, MultiFrameRequest, NodeDataIndex,
    },
    Protocol, TreeSearchBuilder,
};

use crate::multi_frame_classification::MultiCallFrameClassifier;

pub struct AaveV2;
pub struct AaveV3Pool;

impl MultiCallFrameClassifier for AaveV2 {
    const KEY: [u8; 2] = [Protocol::AaveV2 as u8, MultiFrameAction::Liquidation as u8];

    fn create_classifier(
        request: MultiFrameRequest,
    ) -> Option<MultiCallFrameClassification<Action>> {
        Some(MultiCallFrameClassification {
            trace_index:         request.trace_idx,
            tree_search_builder: TreeSearchBuilder::new().with_action(Action::is_transfer),
            parse_fn:            Box::new(parse_v2_v3),
        })
    }
}

impl MultiCallFrameClassifier for AaveV3Pool {
    const KEY: [u8; 2] = [Protocol::AaveV3Pool as u8, MultiFrameAction::Liquidation as u8];

    fn create_classifier(
        request: MultiFrameRequest,
    ) -> Option<MultiCallFrameClassification<Action>> {
        Some(MultiCallFrameClassification {
            trace_index:         request.trace_idx,
            tree_search_builder: TreeSearchBuilder::new().with_action(Action::is_transfer),
            parse_fn:            Box::new(parse_v2_v3),
        })
    }
}

fn parse_v2_v3(this: &mut Action, child_nodes: Vec<(NodeDataIndex, Action)>) -> Vec<NodeDataIndex> {
    let this = this.try_liquidation_mut().unwrap();
    child_nodes
        .into_iter()
        .find_map(|(_, action)| {
            if let Action::Transfer(transfer) = action {
                // because aave has the option to return the Atoken or regular,
                // we can't filter by collateral filter. This might be an issue...
                // tbd tho
                if transfer.to == this.liquidator {
                    this.liquidated_collateral = transfer.amount;
                }
            }

            None
        })
        .map(|e| vec![e])
        .unwrap_or_default()
}
