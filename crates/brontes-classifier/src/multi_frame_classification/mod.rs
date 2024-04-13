//! Multi frame classification is for protocols in which we need more than a
//! single call-frame of context to properly classify. To use this you build a
//! regular classifier that will mark the start node. what will then happen
//! is the tree will mark the index and fetch all child actions of this node
//! and pass this into the multi frame classification.

pub mod one_inch;
pub mod uni_x;

use brontes_types::normalized_actions::{Actions, MultiCallFrameClassification, MultiFrameRequest};
use itertools::Itertools;
use one_inch::OneInchAggregator;
use tracing::warn;

/// for multi call-frame classifier
pub trait MultiCallFrameClassifier {
    /// [self.protocol as u8, self.call_type as u8]
    const KEY: [u8; 2];

    fn create_classifier(
        request: MultiFrameRequest,
    ) -> Option<MultiCallFrameClassification<Actions>>;
}

/// for all of the frame requests, we fetch the underlying function for the
/// given setup
pub fn parse_multi_frame_requests(
    requests: Vec<MultiFrameRequest>,
) -> Vec<MultiCallFrameClassification<Actions>> {
    requests
        .into_iter()
        .filter_map(|request| match request.make_key() {
            OneInchAggregator::KEY => OneInchAggregator::create_classifier(request),
            _ => {
                warn!(?request, "no multi frame classification impl for this request");
                None
            }
        })
        // ensure we go from oldest to newest, this ensures that we process
        // from inner th outer and don't have actions being stolen for nested
        // multi frame classifications
        .sorted_unstable_by(|a, b| b.trace_index.cmp(&a.trace_index))
        .collect()
}
