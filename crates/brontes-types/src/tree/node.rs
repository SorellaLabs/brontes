use reth_primitives::Address;
use tracing::error;

use super::NodeData;
use crate::{normalized_actions::NormalizedAction, TreeSearchArgs, TreeSearchBuilder};

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
        head: u64,
        nodes: &mut NodeData<V>,
    ) {
        if head == self.index {
            let mut results = Vec::new();
            let collect_fn = nodes
                .get_mut(self.data)
                .unwrap()
                .continued_classification_types();

            self.collect(
                &mut results,
                &collect_fn,
                &|a, data| (a.index, data.get_ref(a.data).cloned()),
                nodes,
            );
            let results = results
                .into_iter()
                .filter_map(|(a, b)| Some((a, b?)))
                .collect::<Vec<_>>();

            // Now that we have the child actions of interest we can finalize the parent
            // node's classification which mutates the parents data in place & returns the
            // indexes of child nodes that should be removed
            let prune_collapsed_nodes = nodes
                .get_mut(self.data)
                .unwrap()
                .finalize_classification(results);

            prune_collapsed_nodes.into_iter().for_each(|index| {
                self.remove_node_and_children(index, nodes);
            });

            return
        }

        if self.inner.len() <= 1 {
            if let Some(inner) = self.inner.first_mut() {
                return inner.get_all_children_for_complex_classification(head, nodes)
            }
            error!("was not able to find node in tree");
            return
        }

        let mut iter = self.inner.iter_mut();

        // init the sliding window
        let mut cur_inner_node = iter.next().unwrap();
        let mut next_inner_node = iter.next().unwrap();

        for next_node in iter {
            // check if past nodes are the head
            if cur_inner_node.index == head {
                return cur_inner_node.get_all_children_for_complex_classification(head, nodes)
            } else if next_inner_node.index == head {
                return next_inner_node.get_all_children_for_complex_classification(head, nodes)
            }

            // if the next node is smaller than the head, we continue
            if next_inner_node.index <= head {
                cur_inner_node = next_inner_node;
                next_inner_node = next_node;
            } else {
                // next node is bigger than head. thus current node is proper path
                return cur_inner_node.get_all_children_for_complex_classification(head, nodes)
            }
        }

        // handle case where there are only two inner nodes to look at
        if cur_inner_node.index == head {
            return cur_inner_node.get_all_children_for_complex_classification(head, nodes)
        } else if next_inner_node.index == head {
            return next_inner_node.get_all_children_for_complex_classification(head, nodes)
        } else if next_inner_node.index > head {
            return cur_inner_node.get_all_children_for_complex_classification(head, nodes)
        }
        // handle inf case that is shown in the function docs
        else if let Some(last) = self.inner.last_mut() {
            return last.get_all_children_for_complex_classification(head, nodes)
        }

        error!("was not able to find node in tree, should be unreachable");
    }

    pub fn modify_node_if_contains_childs<F, V: NormalizedAction>(
        &mut self,
        find: &TreeSearchBuilder<V>,
        modify: &F,
        data: &mut NodeData<V>,
    ) -> bool
    where
        F: Fn(&mut Self, &mut NodeData<V>),
    {
        let TreeSearchArgs { collect_current_node, child_node_to_collect } =
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
    pub fn insert(&mut self, n: Node) {
        let trace_addr = n.trace_address.clone();
        self.get_all_inner_nodes(n, trace_addr);
    }

    pub fn get_all_inner_nodes(&mut self, n: Node, mut trace_addr: Vec<usize>) {
        let log = trace_addr.clone();
        if trace_addr.len() == 1 {
            self.inner.push(n);
        } else if let Some(inner) = self.inner.get_mut(trace_addr.remove(0)) {
            inner.get_all_inner_nodes(n, trace_addr)
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

    pub fn remove_node_and_children<V: NormalizedAction>(
        &mut self,
        index: u64,
        data: &mut NodeData<V>,
    ) {
        let mut iter = self.inner.iter_mut().enumerate();

        let res = loop {
            if let Some((i, inner)) = iter.next() {
                if inner.index == index {
                    break Some(i)
                }

                if inner.index < index {
                    inner.remove_node_and_children(index, data)
                } else {
                    break None
                }
            } else {
                break None
            }
        };

        if let Some(val) = res {
            let ret = self.inner.remove(val);
            ret.get_all_sub_actions().into_iter().for_each(|f| {
                data.remove(f);
            });
        }
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
        T: Fn(&Node, &NodeData<V>) -> R,
    {
        let TreeSearchArgs { collect_current_node, child_node_to_collect } =
            call.generate_search_args(self, data);
        if collect_current_node {
            results.push(wanted_data(self, data))
        }

        if child_node_to_collect {
            self.inner
                .iter()
                .for_each(|i| i.collect(results, call, wanted_data, data))
        }
    }
}
