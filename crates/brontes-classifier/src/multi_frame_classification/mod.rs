//! Multi frame classification is for protocols in which we need more than a
//! single call-frame of context to properly classify. To use this you build a
//! regular classifier that will mark the start node. what will then happen
//! is the tree will mark the index and fetch all child actions of this node
//! and pass this into the multi frame classification.

// pub mod one_inch;

use brontes_types::{
    normalized_actions::{Actions, MultiCallFrameClassification, MultiFrameRequest},
    BlockTree,
};

/// for all of the frame requests, we fetch the underlying function for the
/// given setup
pub fn parse_multi_frame_requests(
    requests: Vec<MultiFrameRequest>,
) -> Vec<MultiCallFrameClassification<Actions>> {
}
