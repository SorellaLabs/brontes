//! Multi frame classification is for protocols in which we need more than a
//! single call-frame of context to properly classify. To use this you build a
//! regular classifier that will mark the start node. what will then happen
//! is the tree will mark the index and fetch all child actions of this node
//! and pass this into the multi frame classification.

pub mod aggregator;
pub mod batch;
pub mod flash_loan;
pub mod liquidations;

use aggregator::{OneInchAggregator, OneInchFusion, ZeroXAgg};
use batch::{Cowswap, UniswapX, ZeroXBatch};
use brontes_types::normalized_actions::{Action, MultiCallFrameClassification, MultiFrameRequest};
use flash_loan::{BalancerV2, MakerDss};
use itertools::Itertools;
use liquidations::{AaveV2, AaveV3};
use tracing::debug;

use self::flash_loan::Dodo;

/// for multi call-frame classifier
pub trait MultiCallFrameClassifier {
    /// [self.protocol as u8, self.call_type as u8]
    const KEY: [u8; 2];

    fn create_classifier(
        request: MultiFrameRequest,
    ) -> Option<MultiCallFrameClassification<Action>>;
}

/// for all of the frame requests, we fetch the underlying function for the
/// given setup
pub fn parse_multi_frame_requests(
    requests: Vec<MultiFrameRequest>,
) -> Vec<MultiCallFrameClassification<Action>> {
    requests
        .into_iter()
        .filter_map(|request| match request.make_key() {
            OneInchAggregator::KEY => OneInchAggregator::create_classifier(request),
            OneInchFusion::KEY => OneInchFusion::create_classifier(request),
            UniswapX::KEY => UniswapX::create_classifier(request),
            Cowswap::KEY => Cowswap::create_classifier(request),
            BalancerV2::KEY => BalancerV2::create_classifier(request),
            AaveV2::KEY => AaveV2::create_classifier(request),
            AaveV3::KEY => AaveV3::create_classifier(request),
            ZeroXAgg::KEY => ZeroXAgg::create_classifier(request),
            ZeroXBatch::KEY => ZeroXBatch::create_classifier(request),
            MakerDss::KEY => MakerDss::create_classifier(request),
            Dodo::KEY => Dodo::create_classifier(request),
            _ => {
                debug!(?request, "no multi frame classification impl for this request");
                None
            }
        })
        // ensure we go from oldest to newest, this ensures that we process
        // from inner th outer and don't have actions being stolen for nested
        // multi frame classifications
        .sorted_unstable_by(|a, b| b.trace_index.cmp(&a.trace_index))
        .collect()
}
