use crate::{normalized_actions::NormalizedAction, Node};

pub struct NodeWithDataRef<'a, V: NormalizedAction> {
    pub node: &'a Node,
    pub data: &'a V,
    pub idx:  usize,
}

impl<'a, V: NormalizedAction> NodeWithDataRef<'a, V> {
    pub fn new(node: &'a Node, data: &'a V, idx: usize) -> Self {
        Self { node, data, idx }
    }
}
