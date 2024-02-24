use crate::{normalized_actions::NormalizedAction, Node};

pub struct NodeWithData<V: NormalizedAction> {
    pub node: Node,
    pub data: V,
}

impl<V: NormalizedAction> NodeWithData<V> {
    pub fn new(node: Node, data: V) -> Self {
        Self { node, data }
    }
}

pub struct NodeWithDataRef<'a, V: NormalizedAction> {
    pub node: &'a Node,
    pub data: &'a V,
}

impl<'a, V: NormalizedAction> NodeWithDataRef<'a, V> {
    pub fn new(node: &'a Node, data: &'a V) -> Self {
        Self { node, data }
    }
}

pub struct NodeWithDataMut<'a, V: NormalizedAction> {
    pub node: &'a mut Node,
    pub data: &'a mut V,
}

impl<'a, V: NormalizedAction> NodeWithDataMut<'a, V> {
    pub fn new(node: &'a mut Node, data: &'a mut V) -> Self {
        Self { node, data }
    }
}
