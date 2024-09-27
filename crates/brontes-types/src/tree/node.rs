use itertools::Itertools;
use reth_primitives::Address;
use tracing::{error, warn};

use super::{types::NodeWithDataRef, NodeData};
use crate::{
    normalized_actions::{MultiCallFrameClassification, NodeDataIndex, NormalizedAction},
    TreeSearchArgs, TreeSearchBuilder,
};

#[derive(Debug, Clone)]
pub struct Node {
    pub inner:         Vec<Node>,
    pub finalized:     bool,
    pub index:         u64,
    pub subactions:    Vec<usize>,
    pub trace_address: Vec<usize>,
    pub address:       Address,
    pub data:          usize,
}

impl Node {
    pub fn new(index: u64, address: Address, trace_address: Vec<usize>) -> Self {
        Self {
            index,
            trace_address,
            address,
            finalized: false,
            data: 0,
            inner: vec![],
            subactions: vec![],
        }
    }

    pub fn is_finalized(&self) -> bool {
        self.finalized
    }

    //TODO: Rename & edit docs
    /// Iterates through the tree until the head node is hit. When the head node
    /// is hit, collects all child node actions that are specified by the
    /// head nodes classification types closure.
    /// This works by looking at pairs of child nodes. if the next node has a
    /// index that is greater than our target index. we know that the target
    /// index is contained in the current node. take the following tree:
    ///             0
    ///         /      \
    ///      1          4
    ///   /    \      /  \
    /// 2       3    5     6
    ///
    /// if my target node is 3:
    /// 4 > 3 so go to 1
    /// 1 has 3 as a child. it is found!
    ///
    /// if my target node is 2:
    /// 4 > 2 go to 1
    /// 1 has child 2, it is found!
    ///
    /// if my target node is 6:
    ///   1 < 6, check 4
    ///   4 < 6 check inf
    ///   6 < inf go to 4
    ///   4 has child 6, it is found!
    pub fn get_all_children_for_complex_classification<V: NormalizedAction>(
        &mut self,
        head: &MultiCallFrameClassification<V>,
        nodes: &mut NodeData<V>,
    ) {
        if head.trace_index == self.index {
            let mut results = Vec::new();

            self.collect(
                &mut results,
                head.collect_args(),
                &|data| {
                    (
                        NodeDataIndex {
                            trace_index:    data.node.index,
                            data_idx:       data.node.data as u64,
                            multi_data_idx: data.idx,
                        },
                        data.data.clone(),
                    )
                },
                nodes,
            );

            // should always be the first index
            let this = nodes.get_mut(self.data).unwrap().first_mut().unwrap();
            let clear_collapsed_nodes = head.parse(this, results);

            clear_collapsed_nodes
                .into_iter()
                // remove the outer indexes first to ensure no unreachable
                .sorted_unstable_by(|a, b| b.multi_data_idx.cmp(&a.multi_data_idx))
                .for_each(|index| {
                    self.clear_node_data(index, nodes);
                });

            return
        }

        if self.inner.len() <= 1 {
            if let Some(inner) = self.inner.first_mut() {
                return inner.get_all_children_for_complex_classification(head, nodes)
            }
            warn!("was not able to find node in tree for complex classification");
            return
        }

        let mut iter = self.inner.iter_mut();

        // init the sliding window
        let mut cur_inner_node = iter.next().unwrap();
        let mut next_inner_node = iter.next().unwrap();

        for next_node in iter {
            // check if past nodes are the head
            if cur_inner_node.index == head.trace_index {
                return cur_inner_node.get_all_children_for_complex_classification(head, nodes)
            } else if next_inner_node.index == head.trace_index {
                return next_inner_node.get_all_children_for_complex_classification(head, nodes)
            }

            // if the next node is smaller than the head, we continue
            if next_inner_node.index <= head.trace_index {
                cur_inner_node = next_inner_node;
                next_inner_node = next_node;
            } else {
                // next node is bigger than head. thus current node is proper path
                return cur_inner_node.get_all_children_for_complex_classification(head, nodes)
            }
        }

        // handle case where there are only two inner nodes to look at
        if cur_inner_node.index == head.trace_index {
            return cur_inner_node.get_all_children_for_complex_classification(head, nodes)
        } else if next_inner_node.index == head.trace_index {
            return next_inner_node.get_all_children_for_complex_classification(head, nodes)
        } else if next_inner_node.index > head.trace_index {
            return cur_inner_node.get_all_children_for_complex_classification(head, nodes)
        }
        // handle inf case that is shown in the function docs
        else if let Some(last) = self.inner.last_mut() {
            return last.get_all_children_for_complex_classification(head, nodes)
        }

        warn!("was not able to find node in tree, should be unreachable");
    }

    pub fn modify_node_if_contains_childs<F, V: NormalizedAction>(
        &mut self,
        find: &TreeSearchBuilder<V>,
        modify: &F,
        data: &mut NodeData<V>,
    ) -> bool
    where
        F: Fn(&mut Node, &mut NodeData<V>),
    {
        let TreeSearchArgs { collect_current_node, child_node_to_collect, .. } =
            find.generate_search_args(self, &*data);

        if !child_node_to_collect {
            return false
        }

        let lower_classification_results = self
            .inner
            .iter_mut()
            .map(|node| node.modify_node_if_contains_childs(find, modify, data))
            .collect::<Vec<_>>();

        if !lower_classification_results.into_iter().any(|n| n) {
            // if we don't collect because of parent node
            // we return false
            if collect_current_node {
                modify(self, data);
                return true
            } else {
                return false
            }
        }
        false
    }

    pub fn modify_node_spans<F, V: NormalizedAction>(
        &mut self,
        find: &TreeSearchBuilder<V>,
        modify: &F,
        data: &mut NodeData<V>,
    ) -> bool
    where
        F: Fn(Vec<&mut Self>, &mut NodeData<V>),
    {
        if !find
            .generate_search_args(self, &*data)
            .child_node_to_collect
        {
            return false
        }

        let lower_has_better_collect = self
            .inner
            .iter_mut()
            .map(|n| n.modify_node_spans(find, modify, data))
            .collect::<Vec<_>>();

        // take the collection of nodes that where false and apply modify to that
        // collection

        let all_lower_better = lower_has_better_collect.into_iter().all(|t| t);
        // if all child nodes don't have a best sub-action. Then the current node is the
        // best.
        if !all_lower_better {
            // annoying but only way todo it
            let mut nodes = vec![unsafe { &mut *(self as *mut Self) }];
            for i in &mut self.inner {
                nodes.push(i)
            }

            modify(nodes, data);
        }

        // lower node has a better sub-action.
        true
    }

    pub fn finalize(&mut self) {
        self.finalized = false;
        self.subactions = self.get_all_sub_actions();
        self.finalized = true;

        self.inner.iter_mut().for_each(|f| f.finalize());
    }

    /// The address here is the from address for the trace
    pub fn insert<V: NormalizedAction>(
        &mut self,
        n: Node,
        data: Vec<V>,
        data_store: &mut NodeData<V>,
    ) {
        let trace_addr = n.trace_address.clone();
        self.get_all_inner_nodes(n, data, data_store, trace_addr);
    }

    pub fn get_all_inner_nodes<V: NormalizedAction>(
        &mut self,
        mut n: Node,
        data: Vec<V>,
        data_store: &mut NodeData<V>,
        mut trace_addr: Vec<usize>,
    ) {
        // check if this node is a revert. If it is, we don't insert this node.
        let revert = data_store
            .get_ref(self.data)
            .unwrap()
            .iter()
            .any(|n| n.get_action().is_revert());

        if revert {
            return
        }

        let log = trace_addr.clone();
        if trace_addr.len() == 1 {
            let idx = data_store.add(data);
            n.data = idx;

            self.inner.push(n);
        } else if let Some(inner) = self.inner.get_mut(trace_addr.remove(0)) {
            inner.get_all_inner_nodes(n, data, data_store, trace_addr)
        } else {
            error!("ERROR: {:?}\n {:?}", self.inner, log);
        }
    }

    pub fn get_all_sub_actions(&self) -> Vec<usize> {
        if self.finalized {
            self.subactions.clone()
        } else {
            let mut res = vec![self.data];
            res.extend(
                self.inner
                    .iter()
                    .flat_map(|inner| inner.get_all_sub_actions())
                    .collect::<Vec<_>>(),
            );

            res
        }
    }

    /// doesn't append this node to inner subactions.
    pub fn get_all_sub_actions_exclusive(&self) -> Vec<usize> {
        self.inner
            .iter()
            .flat_map(|inner| inner.get_all_sub_actions())
            .collect::<Vec<_>>()
    }

    /// returns the last create call index
    pub fn get_last_create_call<V: NormalizedAction>(
        &self,
        start_index: &mut u64,
        data_store: &NodeData<V>,
    ) {
        // go through this data setting the index if its a create and happened later
        // than the last index.
        if let Some(this_data) = data_store.get_ref(self.data) {
            for data in this_data {
                if data.is_create() && self.index > *start_index {
                    *start_index = self.index;
                }
            }
        }
        // recursively call lower levels to allow for max index to be found
        for i in &self.inner {
            i.get_last_create_call(start_index, data_store);
        }
    }

    pub fn get_all_parent_nodes_for_discovery(
        &self,
        res: &mut Vec<Node>,
        start_index: u64,
        trace_index: u64,
    ) {
        if self.index >= start_index && self.index < trace_index {
            res.push(self.clone());
            for i in &self.inner {
                i.get_all_parent_nodes_for_discovery(res, start_index, trace_index);
            }
        } else if self.index <= start_index && self.index < trace_index {
            for i in &self.inner {
                i.get_all_parent_nodes_for_discovery(res, start_index, trace_index);
            }
        }
    }

    pub fn get_immediate_parent_node(&self, tx_index: u64) -> Option<&Node> {
        if self.inner.last()?.index == tx_index {
            Some(self)
        } else {
            self.inner.last()?.get_immediate_parent_node(tx_index)
        }
    }

    pub fn tree_right_path(&self) -> Vec<Address> {
        self.inner
            .last()
            .map(|last| {
                let mut last = last.tree_right_path();
                last.push(self.address);
                last
            })
            .unwrap_or(vec![self.address])
    }

    pub fn all_sub_addresses(&self) -> Vec<Address> {
        self.inner
            .iter()
            .flat_map(|i| i.all_sub_addresses())
            .chain(vec![self.address])
            .collect()
    }

    pub fn current_call_stack(&self) -> Vec<Address> {
        let Some(mut stack) = self.inner.last().map(|n| n.current_call_stack()) else {
            return vec![self.address];
        };

        stack.push(self.address);

        stack
    }

    pub fn get_bounded_info<F, R>(&self, lower: u64, upper: u64, res: &mut Vec<R>, info_fn: &F)
    where
        F: Fn(&Node) -> R,
    {
        if self.index >= lower && self.index <= upper {
            res.push(info_fn(self));
        } else {
            return
        }

        self.inner
            .iter()
            .for_each(|node| node.get_bounded_info(lower, upper, res, info_fn));
    }

    /// clears the data for the given node at the specified index. This is used
    /// for complex classification as we want to avoid the double count but
    /// don't want to mess up the structure of the tree.
    pub fn clear_node_data<V: NormalizedAction>(
        &mut self,
        index: NodeDataIndex,
        data: &mut NodeData<V>,
    ) {
        if index.trace_index == self.index {
            data.get_mut(index.data_idx as usize)
                .unwrap()
                .remove(index.multi_data_idx);
            return
        }

        if self.inner.len() <= 1 {
            if let Some(inner) = self.inner.first_mut() {
                return inner.clear_node_data(index, data)
            }
            warn!("was not able to find node in tree for clearing node data");
            return
        }

        let mut iter = self.inner.iter_mut();

        // init the sliding window
        let mut cur_inner_node = iter.next().unwrap();
        let mut next_inner_node = iter.next().unwrap();

        for next_node in iter {
            // check if past nodes are the head
            if cur_inner_node.index == index.trace_index {
                return cur_inner_node.clear_node_data(index, data)
            } else if next_inner_node.index == index.trace_index {
                return next_inner_node.clear_node_data(index, data)
            }

            // if the next node is smaller than the head, we continue
            if next_inner_node.index <= index.trace_index {
                cur_inner_node = next_inner_node;
                next_inner_node = next_node;
            } else {
                // next node is bigger than head. thus current node is proper path
                return cur_inner_node.clear_node_data(index, data)
            }
        }

        // handle case where there are only two inner nodes to look at
        if cur_inner_node.index == index.trace_index {
            return cur_inner_node.clear_node_data(index, data)
        } else if next_inner_node.index == index.trace_index {
            return next_inner_node.clear_node_data(index, data)
        } else if next_inner_node.index > index.trace_index {
            return cur_inner_node.clear_node_data(index, data)
        } else if let Some(last) = self.inner.last_mut() {
            return last.clear_node_data(index, data)
        }

        warn!("was not able to find node in tree, should be unreachable");
    }

    pub fn remove_node_and_children<V: NormalizedAction>(
        &mut self,
        index: u64,
        data: &mut NodeData<V>,
    ) {
        if index == self.index {
            data.remove(self.data);
            self.get_all_sub_actions().into_iter().for_each(|f| {
                data.remove(f);
            });
            return
        }

        if self.inner.len() <= 1 {
            if let Some(inner) = self.inner.first_mut() {
                return inner.remove_node_and_children(index, data)
            }
            warn!("was not able to find node in tree for removing node data");
            return
        }

        let mut iter = self.inner.iter_mut();

        // init the sliding window
        let mut cur_inner_node = iter.next().unwrap();
        let mut next_inner_node = iter.next().unwrap();

        for next_node in iter {
            // check if past nodes are the head
            if cur_inner_node.index == index {
                return cur_inner_node.remove_node_and_children(index, data)
            } else if next_inner_node.index == index {
                return next_inner_node.remove_node_and_children(index, data)
            }

            // if the next node is smaller than the head, we continue
            if next_inner_node.index <= index {
                cur_inner_node = next_inner_node;
                next_inner_node = next_node;
            } else {
                // next node is bigger than head. thus current node is proper path
                return cur_inner_node.remove_node_and_children(index, data)
            }
        }

        // handle case where there are only two inner nodes to look at
        if cur_inner_node.index == index {
            return cur_inner_node.remove_node_and_children(index, data)
        } else if next_inner_node.index == index {
            return next_inner_node.remove_node_and_children(index, data)
        } else if next_inner_node.index > index {
            return cur_inner_node.remove_node_and_children(index, data)
        } else if let Some(last) = self.inner.last_mut() {
            return last.remove_node_and_children(index, data)
        }

        warn!("was not able to find node in tree, should be unreachable");
    }

    // only grabs the lowest subset of specified actions
    pub fn collect_spans<V: NormalizedAction>(
        &self,
        result: &mut Vec<Vec<V>>,
        call: &TreeSearchBuilder<V>,
        data: &NodeData<V>,
    ) -> bool {
        // the previous sub-action was the last one to meet the criteria
        if !call.generate_search_args(self, data).child_node_to_collect {
            return false
        }

        let lower_has_better_collect = self
            .inner
            .iter()
            .map(|i| i.collect_spans(result, call, data))
            .collect::<Vec<bool>>();

        let lower_has_better = lower_has_better_collect.into_iter().all(|f| f);

        // if all child nodes don't have a best sub-action. Then the current node is the
        // best.
        if !lower_has_better {
            let res = self
                .get_all_sub_actions()
                .into_iter()
                .filter_map(|node| data.get_ref(node).cloned())
                .flatten()
                .collect::<Vec<_>>();

            result.push(res);
        }

        // lower node has a better sub-action.
        true
    }

    /// Collects all actions that match the call closure. This is useful for
    /// fetching all actions that match a certain criteria.
    pub fn collect<T, R, V: NormalizedAction>(
        &self,
        results: &mut Vec<R>,
        call: &TreeSearchBuilder<V>,
        wanted_data: &T,
        data: &NodeData<V>,
    ) where
        T: Fn(NodeWithDataRef<'_, V>) -> R,
    {
        let TreeSearchArgs { collect_current_node, child_node_to_collect, collect_idxs } =
            call.generate_search_args(self, data);
        if collect_current_node {
            if let Some(datas) = data.get_ref(self.data) {
                for idx in collect_idxs {
                    results.push(wanted_data(NodeWithDataRef::new(self, &datas[idx], idx)))
                }
            }
        }

        if child_node_to_collect {
            self.inner
                .iter()
                .for_each(|i| i.collect(results, call, wanted_data, data))
        }
    }
}
