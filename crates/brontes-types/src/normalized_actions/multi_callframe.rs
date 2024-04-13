use super::{Actions, NormalizedAction};
use crate::{Protocol, TreeSearchBuilder};

type ParseFn<V> = Box<dyn Fn(&mut V, Vec<(NodeDataIndex, V)>) -> Vec<NodeDataIndex>>;

pub struct MultiCallFrameClassification<V: NormalizedAction> {
    pub trace_index:         u64,
    pub tree_search_builder: TreeSearchBuilder<V>,
    pub parse_fn:            ParseFn<V>,
}

impl<V: NormalizedAction> MultiCallFrameClassification<V> {
    pub fn parse(&self, this: &mut V, actions: Vec<(NodeDataIndex, V)>) -> Vec<NodeDataIndex> {
        (self.parse_fn)(this, actions)
    }

    pub fn collect_args(&self) -> &TreeSearchBuilder<V> {
        &self.tree_search_builder
    }
}

#[derive(Debug, Clone, Copy)]
pub struct NodeDataIndex {
    /// the index of the call-frame trace in the tree
    pub trace_index:    u64,
    /// the index of the node data struct
    pub data_idx:       u64,
    /// the index for the vec that we get from the node data struct
    pub multi_data_idx: usize,
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum MultiFrameAction {
    FlashLoan,
    Batch,
    Liquidation,
    Aggregator,
}

#[derive(Debug, Clone, Copy)]
pub struct MultiFrameRequest {
    pub protocol:  Protocol,
    pub call_type: MultiFrameAction,
    pub trace_idx: u64,
}

impl MultiFrameRequest {
    pub fn new(action: &Actions, trace_idx: u64) -> Option<Self> {
        match action {
            Actions::FlashLoan(f) => Some(Self {
                protocol: f.protocol,
                call_type: MultiFrameAction::FlashLoan,
                trace_idx,
            }),
            Actions::Batch(b) => {
                Some(Self { protocol: b.protocol, call_type: MultiFrameAction::Batch, trace_idx })
            }
            Actions::Liquidation(l) => Some(Self {
                protocol: l.protocol,
                call_type: MultiFrameAction::Liquidation,
                trace_idx,
            }),
            Actions::Aggregator(a) => Some(Self {
                protocol: a.protocol,
                call_type: MultiFrameAction::Aggregator,
                trace_idx,
            }),
            _ => None,
        }
    }

    pub const fn make_key(&self) -> [u8; 2] {
        [self.protocol as u8, self.call_type as u8]
    }
}
