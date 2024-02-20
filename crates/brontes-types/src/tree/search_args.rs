use std::marker::PhantomData;

use alloy_primitives::Address;

use crate::{normalized_actions::NormalizedAction, Node, NodeData};

#[derive(Debug, Clone, Default, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TreeSearchArgs {
    pub collect_current_node: bool,
    pub child_node_to_collect: bool,
}

pub struct TreeSearchBuilder<V: NormalizedAction> {
    /// these get or'd together
    with_actions: Vec<usize>,
    /// get or'd together with contains
    child_node_have: Vec<usize>,
    /// gets and'd together
    child_nodes_contains: Vec<usize>,
    /// gets and'd together
    has_address: Option<Address>,
    _p: PhantomData<V>,
}
impl<V: NormalizedAction> Default for TreeSearchBuilder<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V: NormalizedAction> TreeSearchBuilder<V> {
    pub fn new() -> Self {
        Self {
            with_actions: vec![],
            child_node_have: vec![],
            child_nodes_contains: vec![],
            has_address: None,
            _p: Default::default(),
        }
    }
    /// takes a function ptr and will or the result with other defined functions
    pub fn with_action(mut self, action_fn: fn(&V) -> bool) -> Self {
        let ptr = action_fn as usize;
        self.with_actions.push(ptr);
        self
    }

    /// takes a function ptrs and will or the result with other defined functions
    pub fn with_actions<const N: usize>(mut self, action_fns: [fn(&V) -> bool; N]) -> Self {
        self.with_actions
            .extend(action_fns.into_iter().map(|f| f as usize));
        self
    }

    /// when searching for child nodes, makes sure that there is atleast one of the following
    /// actions defined by the args
    pub fn child_nodes_have<const N: usize>(mut self, action_fns: [fn(V) -> bool; N]) -> Self {
        self.child_node_have = action_fns.into_iter().map(|f| f as usize).collect();
        self
    }

    /// when searching for child nodes. will check that the tree has the entire set of different
    /// actions, specified by this args
    pub fn child_nodes_contain<const N: usize>(mut self, action_fns: [fn(V) -> bool; N]) -> Self {
        self.child_nodes_contains = action_fns.into_iter().map(|f| f as usize).collect();
        self
    }

    pub fn with_address(mut self, address: Address) -> Self {
        self.has_address = Some(address);
        self
    }

    pub fn generate_search_args(&self, node: &Node, node_data: &NodeData<V>) -> TreeSearchArgs {
        let collect_current_node = self.collect_current_node(node, node_data);
        let child_node_to_collect =
            if self.child_nodes_contains.is_empty() && self.child_node_have.is_empty() {
                self.has_child_nodes_default(node, node_data)
            } else {
                let (all, has_addr, have_any) = self.has_child_nodes(node, node_data);

                if self.has_address.is_some() {
                    have_any & all & has_addr
                } else {
                    have_any & all
                }
            };

        TreeSearchArgs {
            collect_current_node,
            child_node_to_collect,
        }
    }

    fn collect_current_node(&self, node: &Node, node_data: &NodeData<V>) -> bool {
        node_data
            .get_ref(node.data)
            .map(|node_action| {
                self.with_actions
                    .iter()
                    .map(|ptr| {
                        let ptr = *ptr as *const ();
                        let ptr: fn(&V) -> bool = unsafe { std::mem::transmute(ptr) };
                        ptr(node_action)
                    })
                    .reduce(|a, b| a | b)
                    .unwrap_or(false)
                    | self
                        .has_address
                        .map(|addr| node_action.get_action().get_to_address() == addr)
                        .unwrap_or_default()
            })
            .unwrap_or_default()
    }

    fn has_child_nodes_default(&self, node: &Node, node_data: &NodeData<V>) -> bool {
        node.get_all_sub_actions()
            .iter()
            .filter_map(|node| node_data.get_ref(*node))
            .any(|action| {
                self.with_actions
                    .iter()
                    .map(|ptr| {
                        let ptr = *ptr as *const ();
                        let ptr: fn(&V) -> bool = unsafe { std::mem::transmute(ptr) };
                        ptr(action)
                    })
                    .reduce(|a, b| a | b)
                    .unwrap_or(false)
                    | self
                        .has_address
                        .map(|addr| action.get_action().get_to_address() == addr)
                        .unwrap_or_default()
            })
    }

    fn has_child_nodes(&self, node: &Node, node_data: &NodeData<V>) -> (bool, bool, bool) {
        let mut all = Vec::new();
        for _ in 0..self.child_nodes_contains.len() {
            all.push(false);
        }

        let mut have_any = false;
        let mut has_addr = false;

        node.get_all_sub_actions()
            .iter()
            .filter_map(|node| node_data.get_ref(*node))
            .for_each(|action| {
                // for have, its a or with the result
                have_any |= self
                    .child_node_have
                    .iter()
                    .map(|ptr| {
                        let ptr = *ptr as *const ();
                        let ptr: fn(&V) -> bool = unsafe { std::mem::transmute(ptr) };
                        ptr(action)
                    })
                    .reduce(|a, b| a | b)
                    .unwrap_or_default();

                self.child_nodes_contains
                    .iter()
                    .enumerate()
                    .for_each(|(i, ptr)| {
                        let ptr = *ptr as *const ();
                        let ptr: fn(&V) -> bool = unsafe { std::mem::transmute(ptr) };
                        all[i] |= ptr(action);
                    });

                // check if has addr
                if let Some(addr) = self.has_address {
                    has_addr |= action.get_action().get_to_address() == addr;
                }
            });

        // allows us to & these together
        let all = if all.is_empty() {
            true
        } else {
            all.iter().all(|a| *a)
        };

        (all, has_addr, have_any)
    }
}
