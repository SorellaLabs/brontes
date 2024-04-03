use alloy_primitives::Address;

use crate::{tree::NormalizedAction, Node, NodeData};

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct TreeSearchArgs {
    pub collect_current_node:  bool,
    pub collect_idxs:          Vec<usize>,
    pub child_node_to_collect: bool,
}

#[derive(Debug, Clone)]
pub struct TreeSearchBuilder<V: NormalizedAction> {
    /// these get or'd together
    with_actions:         Vec<fn(&V) -> bool>,
    /// get or'd together with contains
    child_node_have:      Vec<fn(&V) -> bool>,
    /// gets and'd together
    child_nodes_contains: Vec<fn(&V) -> bool>,
    /// gets and'd together
    has_from_address:     Option<Address>,
    /// gets and'd together
    has_to_address:       Option<Vec<Address>>,
}
impl<V: NormalizedAction> Default for TreeSearchBuilder<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V: NormalizedAction> TreeSearchBuilder<V> {
    pub fn new() -> Self {
        Self {
            with_actions:         vec![],
            child_node_have:      vec![],
            child_nodes_contains: vec![],
            has_from_address:     None,
            has_to_address:       None,
        }
    }

    /// Will collect all actions that the search passes if it is equal to the
    /// given function arg. if no child node search args are set. The search
    /// will use this action as the default.
    pub fn with_action(mut self, action_fn: fn(&V) -> bool) -> Self {
        self.with_actions.push(action_fn);
        self
    }

    /// Will collect all actions that the search passes if it equals one of the
    /// function args passed in. If no child node search args are set. These
    /// action fn will be used to search for child nodes
    pub fn with_actions<const N: usize>(mut self, action_fns: [fn(&V) -> bool; N]) -> Self {
        self.with_actions.extend(action_fns);
        self
    }

    /// When searching for child nodes, makes sure that there is atleast one of
    /// the following actions defined by the given functions
    pub fn child_nodes_have<const H: usize>(mut self, action_fns: [fn(&V) -> bool; H]) -> Self {
        if !self.child_nodes_contains.is_empty() {
            tracing::error!(
                "child nodes contains already set, only one of contains, or have is allowed"
            );
            return self
        }

        self.child_node_have = action_fns.to_vec();
        self
    }

    /// When searching for child nodes, makes sure that there is all of the
    /// following actions defined by the given functions
    pub fn child_nodes_contain<const C: usize>(mut self, action_fns: [fn(&V) -> bool; C]) -> Self {
        if !self.child_node_have.is_empty() {
            tracing::error!(
                "child nodes contains already set, only one of contains, or have is allowed"
            );
            return self
        }
        self.child_nodes_contains = action_fns.to_vec();
        self
    }

    /// There can only be 1 address set currently. When this is set.
    /// only nodes that have this address + any other arguments specified will
    /// be collected.
    pub fn with_from_address(mut self, address: Address) -> Self {
        self.has_from_address = Some(address);
        self
    }

    pub fn with_to_address(mut self, addresses: Vec<Address>) -> Self {
        self.has_to_address = Some(addresses);
        self
    }

    pub fn generate_search_args(&self, node: &Node, node_data: &NodeData<V>) -> TreeSearchArgs {
        let (collect_current_node, collect_idxs) = self.collect_current_node(node, node_data);
        let child_node_to_collect =
            if self.child_nodes_contains.is_empty() && self.child_node_have.is_empty() {
                self.has_child_nodes_default(node, node_data)
            } else {
                self.has_child_nodes(node, node_data)
            };

        TreeSearchArgs { collect_current_node, child_node_to_collect, collect_idxs }
    }

    fn collect_current_node(&self, node: &Node, node_data: &NodeData<V>) -> (bool, Vec<usize>) {
        node_data
            .get_ref(node.data)
            .map(|node_actions| {
                let (is_good, idxs): (Vec<_>, Vec<_>) = node_actions
                    .iter()
                    .enumerate()
                    .map(|(idx, node_action)| {
                        let is_true = self
                            .with_actions
                            .iter()
                            .map(|ptr| {
                                ptr(node_action)
                                    && self
                                        .has_from_address
                                        .map(|addr| {
                                            node_action.get_action().get_from_address() == addr
                                        })
                                        .unwrap_or(true)
                                    && self
                                        .has_to_address
                                        .as_ref()
                                        .map(|addrs| {
                                            addrs.contains(
                                                &node_action.get_action().get_to_address(),
                                            )
                                        })
                                        .unwrap_or(true)
                            })
                            .reduce(|a, b| a | b)
                            .unwrap_or(false);

                        if is_true {
                            (true, idx)
                        } else {
                            // rand number but shoudl be big enough
                            (false, 694200)
                        }
                    })
                    .unzip();
                (
                    is_good.iter().any(|f| *f),
                    idxs.into_iter()
                        .filter(|idx| *idx != 694200)
                        .collect::<Vec<_>>(),
                )
            })
            .unwrap_or_default()
    }

    fn has_child_nodes_default(&self, node: &Node, node_data: &NodeData<V>) -> bool {
        node.get_all_sub_actions()
            .iter()
            .filter_map(|node| node_data.get_ref(*node))
            .flatten()
            .any(|action| {
                self.with_actions
                    .iter()
                    .map(|ptr| {
                        ptr(action)
                            && self
                                .has_from_address
                                .map(|addr| action.get_action().get_from_address() == addr)
                                .unwrap_or(true)
                            && self
                                .has_to_address
                                .as_ref()
                                .map(|addrs| addrs.contains(&action.get_action().get_to_address()))
                                .unwrap_or(true)
                    })
                    .reduce(|a, b| a | b)
                    .unwrap_or(false)
            })
    }

    fn has_child_nodes(&self, node: &Node, node_data: &NodeData<V>) -> bool {
        let mut all = Vec::new();
        all.resize(self.child_nodes_contains.len(), false);
        let mut have_any = false;

        node.get_all_sub_actions()
            .iter()
            .filter_map(|node| node_data.get_ref(*node))
            .flatten()
            .for_each(|action| {
                // for have, its a or with the result
                have_any |= self
                    .child_node_have
                    .iter()
                    .map(|ptr| {
                        ptr(action)
                            && self
                                .has_from_address
                                .map(|addr| action.get_action().get_from_address() == addr)
                                .unwrap_or(true)
                            && self
                                .has_to_address
                                .as_ref()
                                .map(|addrs| addrs.contains(&action.get_action().get_to_address()))
                                .unwrap_or(true)
                    })
                    .reduce(|a, b| a | b)
                    .unwrap_or_default();

                self.child_nodes_contains
                    .iter()
                    .enumerate()
                    .for_each(|(i, ptr)| {
                        all[i] |= ptr(action);
                    });
            });

        // allows us to & these together
        let all = if all.is_empty() { true } else { all.iter().all(|a| *a) };

        let has_any = if self.child_node_have.is_empty() { true } else { have_any };

        all & has_any
    }
}
