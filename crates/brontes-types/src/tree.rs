use std::collections::{HashMap, HashSet};

use rayon::{
    iter::IntoParallelIterator,
    prelude::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator},
};
use redefined::{self_convert_redefined, RedefinedConvert};
use reth_primitives::{Address, Header, B256};
use serde::{Deserialize, Serialize};
use sorella_db_databases::clickhouse::{self, Row};
use tracing::error;

use crate::normalized_actions::NormalizedAction;

#[derive(Debug, Serialize, Deserialize)]
pub struct BlockTree<V: NormalizedAction> {
    pub tx_roots:         Vec<Root<V>>,
    pub header:           Header,
    pub avg_priority_fee: u128,
}

impl<V: NormalizedAction> BlockTree<V> {
    pub fn new(header: Header, tx_num: usize) -> Self {
        Self { tx_roots: Vec::with_capacity(tx_num), header, avg_priority_fee: 0 }
    }

    pub fn get_root(&self, tx_hash: B256) -> Option<&Root<V>> {
        self.tx_roots.par_iter().find_any(|r| r.tx_hash == tx_hash)
    }

    pub fn get_gas_details(&self, hash: B256) -> Option<&GasDetails> {
        self.tx_roots
            .iter()
            .find(|h| h.tx_hash == hash)
            .map(|root| &root.gas_details)
    }

    pub fn get_prev_tx(&self, hash: B256) -> B256 {
        let (index, _) = self
            .tx_roots
            .iter()
            .enumerate()
            .find(|(_, h)| h.tx_hash == hash)
            .unwrap();

        self.tx_roots[index - 1].tx_hash
    }

    pub fn insert_root(&mut self, root: Root<V>) {
        self.tx_roots.push(root);
    }

    pub fn finalize_tree(&mut self) {
        // because of this bad boy: https://etherscan.io/block/18500239
        // we need this
        if self.tx_roots.is_empty() {
            error!(block = self.header.number, "have empty tree");
            self.tx_roots.iter_mut().for_each(|root| root.finalize());
            return
        }

        self.avg_priority_fee = self
            .tx_roots
            .iter()
            .map(|tx| {
                tx.gas_details.effective_gas_price - self.header.base_fee_per_gas.unwrap() as u128
            })
            .sum::<u128>()
            / self.tx_roots.len() as u128;

        self.tx_roots.iter_mut().for_each(|root| root.finalize());
    }

    pub fn insert_node(&mut self, node: Node<V>) {
        self.tx_roots
            .last_mut()
            .expect("no root_nodes inserted")
            .insert(node);
    }

    pub fn get_hashes(&self) -> Vec<B256> {
        self.tx_roots.iter().map(|r| r.tx_hash).collect()
    }

    /// Collects all subsets of actions that match the action criteria specified
    /// by the closure. This is useful for collecting the subtrees of a
    /// transaction that contain the wanted actions.
    pub fn collect_spans<F>(&self, hash: B256, call: F) -> Vec<Vec<V>>
    where
        F: Fn(&Node<V>) -> bool,
    {
        if let Some(root) = self.tx_roots.iter().find(|r| r.tx_hash == hash) {
            root.collect_spans(&call)
        } else {
            vec![]
        }
    }

    pub fn collect<F>(&self, hash: B256, call: F) -> Vec<V>
    where
        F: Fn(&Node<V>) -> (bool, bool) + Send + Sync,
    {
        if let Some(root) = self.tx_roots.iter().find(|r| r.tx_hash == hash) {
            root.collect(&call)
        } else {
            vec![]
        }
    }

    pub fn collect_all<F>(&self, call: F) -> HashMap<B256, Vec<V>>
    where
        F: Fn(&Node<V>) -> (bool, bool) + Send + Sync,
    {
        self.tx_roots
            .par_iter()
            .map(|r| (r.tx_hash, r.collect(&call)))
            .collect()
    }

    /// Takes Vec<(TransactionIndex, Vec<ActionIndex>)>
    /// for every action index of a transaction index, This function grabs all
    /// child nodes of the action index if and only if they are specified in
    /// the classification function of the action index node.
    pub fn collect_and_classify(&mut self, search_params: &Vec<Option<(usize, Vec<u64>)>>) {
        let mut roots_with_search_params = self
            .tx_roots
            .iter_mut()
            .zip(search_params.iter())
            .collect::<Vec<_>>();

        roots_with_search_params
            .par_iter_mut()
            .filter_map(|(root, opt)| Some((root, opt.as_ref()?)))
            .for_each(|(root, (_, subtraces))| {
                root.collect_child_traces_and_classify(subtraces);
            });
    }

    /// Collects all subsets of actions that match the action criteria specified
    /// by the closure. This is useful for collecting the subtrees of a
    /// transaction that contain the wanted actions.
    pub fn collect_spans_all<F>(&self, call: F) -> HashMap<B256, Vec<Vec<V>>>
    where
        F: Fn(&Node<V>) -> bool + Send + Sync,
    {
        self.tx_roots
            .par_iter()
            .map(|r| (r.tx_hash, r.collect_spans(&call)))
            .collect()
    }

    //TODO: (Will) Write the docs for this
    /// The first function parses down the tree to the point where we
    /// are at the lowest subset of the valid action. It then the dynamically
    /// decodes the call gets executed in order to capture the
    pub fn dyn_classify<T, F>(&mut self, find: T, call: F) -> Vec<(Address, (Address, Address))>
    where
        T: Fn(Address, &Node<V>) -> (bool, bool) + Sync,
        F: Fn(&mut Node<V>) -> Option<(Address, (Address, Address))> + Send + Sync,
    {
        self.tx_roots
            .par_iter_mut()
            .flat_map(|root| root.dyn_classify(&find, &call))
            .collect()
    }

    pub fn modify_node_if_contains_childs<T, F>(&mut self, find: T, modify: F)
    where
        T: Fn(&Node<V>) -> (bool, bool) + Send + Sync,
        F: Fn(&mut Node<V>) + Send + Sync,
    {
        self.tx_roots
            .par_iter_mut()
            .for_each(|r| r.modify_node_if_contains_childs(&find, &modify));
    }

    pub fn remove_duplicate_data<FindActionHead, FindRemoval, ClassifyRemovalIndex, WantedData, R>(
        &mut self,
        find: FindActionHead,
        find_removal: FindRemoval,
        info: WantedData,
        classify: ClassifyRemovalIndex,
    ) where
        WantedData: Fn(&Node<V>) -> R + Sync,
        ClassifyRemovalIndex: Fn(&Vec<R>, &Node<V>) -> Vec<u64> + Sync,
        FindActionHead: Fn(&Node<V>) -> (bool, bool) + Sync,
        FindRemoval: Fn(&Node<V>) -> (bool, bool) + Sync,
    {
        self.tx_roots
            .par_iter_mut()
            .for_each(|root| root.remove_duplicate_data(&find, &classify, &info, &find_removal));
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Root<V: NormalizedAction> {
    pub head:        Node<V>,
    pub position:    usize,
    pub tx_hash:     B256,
    pub private:     bool,
    pub gas_details: GasDetails,
}

impl<V: NormalizedAction> Root<V> {
    pub fn get_block_position(&self) -> usize {
        self.position
    }

    pub fn insert(&mut self, node: Node<V>) {
        self.head.insert(node)
    }

    pub fn collect_spans<F>(&self, call: &F) -> Vec<Vec<V>>
    where
        F: Fn(&Node<V>) -> bool,
    {
        let mut result = Vec::new();
        self.head.collect_spans(&mut result, call);

        result
    }

    pub fn collect<F>(&self, call: &F) -> Vec<V>
    where
        F: Fn(&Node<V>) -> (bool, bool),
    {
        let mut result = Vec::new();
        self.head
            .collect(&mut result, call, &|data| data.data.clone());

        result
    }

    pub fn modify_node_if_contains_childs<T, F>(&mut self, find: &T, modify: &F)
    where
        T: Fn(&Node<V>) -> (bool, bool),
        F: Fn(&mut Node<V>),
    {
        self.head.modify_node_if_contains_childs(find, modify);
    }

    pub fn collect_child_traces_and_classify(&mut self, heads: &Vec<u64>) {
        heads.into_iter().for_each(|search_head| {
            self.head
                .get_all_children_for_complex_classification(*search_head)
        });
    }

    pub fn remove_duplicate_data<F, C, T, R, Re>(
        &mut self,
        find: &F,
        classify: &C,
        info: &T,
        removal: &Re,
    ) where
        T: Fn(&Node<V>) -> R + Sync,
        C: Fn(&Vec<R>, &Node<V>) -> Vec<u64> + Sync,
        F: Fn(&Node<V>) -> (bool, bool),
        Re: Fn(&Node<V>) -> (bool, bool) + Sync,
    {
        let mut find_res = Vec::new();
        self.head.collect(&mut find_res, find, &|data| data.clone());

        let indexes = find_res
            .into_par_iter()
            .flat_map(|node| {
                let mut bad_res = Vec::new();
                node.collect(&mut bad_res, removal, info);
                classify(&bad_res, &node)
            })
            .collect::<HashSet<_>>();

        indexes
            .into_iter()
            .for_each(|index| self.head.remove_node_and_children(index));
    }

    pub fn dyn_classify<T, F>(&mut self, find: &T, call: &F) -> Vec<(Address, (Address, Address))>
    where
        T: Fn(Address, &Node<V>) -> (bool, bool),
        F: Fn(&mut Node<V>) -> Option<(Address, (Address, Address))> + Send + Sync,
    {
        // bool is used for recursion
        let mut results = Vec::new();
        let _ = self.head.dyn_classify(find, call, &mut results);

        results
    }

    pub fn finalize(&mut self) {
        self.head.finalize();
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    Serialize,
    Deserialize,
    Row,
    Default,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
)]
pub struct GasDetails {
    pub coinbase_transfer:   Option<u128>,
    pub priority_fee:        u128,
    pub gas_used:            u128,
    pub effective_gas_price: u128,
}

self_convert_redefined!(GasDetails);

impl GasDetails {
    pub fn gas_paid(&self) -> u128 {
        let mut gas = self.gas_used * self.effective_gas_price;

        if let Some(coinbase) = self.coinbase_transfer {
            gas += coinbase
        }

        gas
    }

    pub fn priority_fee(&self, base_fee: u128) -> u128 {
        self.effective_gas_price - base_fee
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Node<V: NormalizedAction> {
    pub inner:     Vec<Node<V>>,
    pub finalized: bool,
    pub index:     u64,

    /// This only has values when the node is frozen
    pub subactions:    Vec<V>,
    pub trace_address: Vec<usize>,
    pub address:       Address,
    pub data:          V,
}

impl<V: NormalizedAction> Node<V> {
    pub fn new(index: u64, address: Address, data: V, trace_address: Vec<usize>) -> Self {
        Self {
            index,
            trace_address,
            address,
            finalized: false,
            data,
            inner: vec![],
            subactions: vec![],
        }
    }

    pub fn is_finalized(&self) -> bool {
        self.finalized
    }

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
    pub fn get_all_children_for_complex_classification(&mut self, head: u64) {
        if head == self.index {
            let mut results = Vec::new();
            let classification = self.data.continued_classification_types();

            let collect_fn = |node: &Node<V>| {
                (
                    (classification)(&node.data),
                    node.get_all_sub_actions()
                        .iter()
                        .any(|i| (classification)(i)),
                )
            };
            self.collect(&mut results, &collect_fn, &|a| (a.index, a.data.clone()));
            // Now that we have the child actions of interest we can finalize the parent
            // node's classification which mutates the parents data in place & returns the
            // indexes of child nodes that should be removed
            let prune_collapsed_nodes = self.data.finalize_classification(results);

            prune_collapsed_nodes.into_iter().for_each(|index| {
                self.remove_node_and_children(index);
            });

            return
        }

        if self.inner.len() <= 1 {
            if let Some(inner) = self.inner.first_mut() {
                return inner.get_all_children_for_complex_classification(head)
            }
            error!("was not able to find node in tree");
            return
        }

        let mut iter = self.inner.iter_mut();

        // init the sliding window
        let mut cur_inner_node = iter.next().unwrap();
        let mut next_inner_node = iter.next().unwrap();

        while let Some(next_node) = iter.next() {
            // check if past nodes are the head
            if cur_inner_node.index == head {
                return cur_inner_node.get_all_children_for_complex_classification(head)
            } else if next_inner_node.index == head {
                return next_inner_node.get_all_children_for_complex_classification(head)
            }

            // if the next node is smaller than the head, we continue
            if next_inner_node.index <= head {
                cur_inner_node = next_inner_node;
                next_inner_node = next_node;
            } else {
                // next node is bigger than head. thus current node is proper path
                return cur_inner_node.get_all_children_for_complex_classification(head)
            }
        }

        // handle case where there are only two inner nodes to look at
        if cur_inner_node.index == head {
            return cur_inner_node.get_all_children_for_complex_classification(head)
        } else if next_inner_node.index == head {
            return next_inner_node.get_all_children_for_complex_classification(head)
        } else if next_inner_node.index > head {
            return cur_inner_node.get_all_children_for_complex_classification(head)
        }
        // handle inf case that is shown in the function docs
        else if let Some(last) = self.inner.last_mut() {
            return last.get_all_children_for_complex_classification(head)
        }

        error!("was not able to find node in tree, should be unreachable");
    }

    pub fn modify_node_if_contains_childs<T, F>(&mut self, find: &T, modify: &F) -> bool
    where
        T: Fn(&Self) -> (bool, bool),
        F: Fn(&mut Self),
    {
        let (is_parent_node, has_lower_set) = find(&self);
        if !has_lower_set {
            return false
        }

        let lower_classification_results = self
            .inner
            .iter_mut()
            .map(|node| node.modify_node_if_contains_childs(find, modify))
            .collect::<Vec<_>>();

        if !lower_classification_results.into_iter().any(|n| n) {
            // if we don't collect because of parent node
            // we return false
            if is_parent_node {
                modify(self);
                return true
            } else {
                return false
            }
        }
        false
    }

    pub fn finalize(&mut self) {
        self.finalized = false;
        self.subactions = self.get_all_sub_actions();
        self.finalized = true;

        self.inner.iter_mut().for_each(|f| f.finalize());
    }

    /// The address here is the from address for the trace
    pub fn insert(&mut self, n: Node<V>) {
        let trace_addr = n.trace_address.clone();
        self.get_all_inner_nodes(n, trace_addr);
    }

    pub fn get_all_inner_nodes(&mut self, n: Node<V>, mut trace_addr: Vec<usize>) {
        let log = trace_addr.clone();
        if trace_addr.len() == 1 {
            self.inner.push(n);
        } else if let Some(inner) = self.inner.get_mut(trace_addr.remove(0)) {
            inner.get_all_inner_nodes(n, trace_addr)
        } else {
            error!("ERROR: {:?}\n {:?}", self.inner, log);
        }
    }

    pub fn get_all_sub_actions(&self) -> Vec<V> {
        if self.finalized {
            self.subactions.clone()
        } else {
            let mut res = vec![self.data.clone()];
            res.extend(
                self.inner
                    .iter()
                    .flat_map(|inner| inner.get_all_sub_actions())
                    .collect::<Vec<V>>(),
            );

            res
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
            return vec![self.address]
        };

        stack.push(self.address);

        stack
    }

    pub fn get_bounded_info<F, R>(&self, lower: u64, upper: u64, res: &mut Vec<R>, info_fn: &F)
    where
        F: Fn(&Node<V>) -> R,
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

    //TODO: Will docs pls
    pub fn remove_node_and_children(&mut self, index: u64) {
        let mut iter = self.inner.iter_mut().enumerate();

        let res = loop {
            if let Some((i, inner)) = iter.next() {
                if inner.index == index {
                    break Some(i)
                }

                if inner.index < index {
                    inner.remove_node_and_children(index)
                } else {
                    break None
                }
            } else {
                break None
            }
        };

        if let Some(val) = res {
            self.inner.remove(val);
        }
    }

    // only grabs the lowest subset of specified actions
    pub fn collect_spans<F>(&self, result: &mut Vec<Vec<V>>, call: &F) -> bool
    where
        F: Fn(&Node<V>) -> bool,
    {
        // the previous sub-action was the last one to meet the criteria
        if !call(self) {
            return false
        }

        let lower_has_better_collect = self
            .inner
            .iter()
            .map(|i| i.collect_spans(result, call))
            .collect::<Vec<bool>>();

        let lower_has_better = lower_has_better_collect.into_iter().any(|f| f);

        // if all child nodes don't have a best sub-action. Then the current node is the
        // best.
        if !lower_has_better {
            let res = self.get_all_sub_actions();
            result.push(res);
        }

        // lower node has a better sub-action.
        true
    }

    // will collect all elements of the operation that are specified.
    // useful for fetching all transfers etc
    pub fn collect<F, T, R>(&self, results: &mut Vec<R>, call: &F, wanted_data: &T)
    where
        F: Fn(&Node<V>) -> (bool, bool),
        T: Fn(&Node<V>) -> R,
    {
        let (add, go_lower) = call(self);
        if add {
            results.push(wanted_data(self))
        }

        if go_lower {
            self.inner
                .iter()
                .for_each(|i| i.collect(results, call, wanted_data))
        }
    }

    pub fn dyn_classify<T, F>(
        &mut self,
        find: &T,
        call: &F,
        result: &mut Vec<(Address, (Address, Address))>,
    ) -> bool
    where
        T: Fn(Address, &Node<V>) -> (bool, bool),
        F: Fn(&mut Node<V>) -> Option<(Address, (Address, Address))> + Send + Sync,
    {
        let (go_lower, set_change) = find(self.address, self);

        if !go_lower {
            return false
        }

        if set_change {
            if let Some(res) = call(self) {
                result.push(res);
            }
        }

        let lower_has_better_c = self
            .inner
            .iter_mut()
            .map(|i| i.dyn_classify(find, call, result))
            .collect::<Vec<_>>();

        let lower_has_better = lower_has_better_c.into_iter().any(|i| i);

        if !lower_has_better && !set_change {
            if let Some(res) = call(self) {
                result.push(res);
            }
        }

        true
    }
}

/*
#[cfg(test)]
mod tests {

    use std::env;

    use brontes_classifier::test_utils::build_raw_test_tree;
    use brontes_core::{decoding::parser::TraceParser, test_utils::init_trace_parser};
    use brontes_database::clickhouse::Clickhouse;
    use brontes_database::libmdbx::Libmdbx;
    use reth_primitives::{revm_primitives::db::Database, Address};
    use reth_rpc_types::trace::parity::{TraceType, TransactionTrace};
    use serial_test::serial;
    use tokio::sync::mpsc::unbounded_channel;

    use super::*;
    use crate::{normalized_actions::Actions, test_utils::force_call_action, tree::Node};

    #[derive(Debug, PartialEq, Eq)]
    pub struct ComparisonNode {
        inner_len:      usize,
        finalized:      bool,
        index:          u64,
        subactions_len: usize,
        trace_address:  Vec<usize>,
        address:        Address,
        trace:          TransactionTrace,
    }

    impl ComparisonNode {
        pub fn new(trace: &TransactionTrace, index: usize, inner_len: usize) -> Self {
            Self {
                inner_len,
                finalized: false,
                index: index as u64,
                subactions_len: 0,
                trace_address: trace.trace_address.clone(),
                address: force_call_action(trace).from,
                trace: trace.clone(),
            }
        }
    }

    impl From<&Node<Actions>> for ComparisonNode {
        fn from(value: &Node<Actions>) -> Self {
            ComparisonNode {
                inner_len:      value.inner.len(),
                finalized:      value.finalized,
                index:          value.index,
                subactions_len: value.subactions.len(),
                trace_address:  value.trace_address.clone(),
                address:        value.address,
                trace:          match &value.data {
                    Actions::Unclassified(traces) => traces.trace.clone(),
                    _ => unreachable!(),
                },
            }
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_raw_tree() {
        let block_num = 18180900;
        dotenv::dotenv().ok();

        let (tx, _rx) = unbounded_channel();
        let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
        let libmdbx = Libmdbx::init_db(brontes_db_endpoint, None).unwrap();

        let tracer = init_trace_parser(tokio::runtime::Handle::current().clone(), tx, &libmdbx);
        let db = Clickhouse::default();
        let mut tree = build_raw_test_tree(&tracer, &db, block_num).await;

        // let mut transaction_traces = tracer
        //     .tracer
        //     .trace
        //     .replay_block_transactions(block_num.into(),
        // HashSet::from([TraceType::Trace]))     .await
        //     .unwrap()
        //     .unwrap();
        // assert_eq!(tree.roots.len(), transaction_traces.len());
        //
        // let first_root = tree.roots.remove(0);
        // let first_tx = transaction_traces.remove(0);
        /*

            assert_eq!(
                ComparisonNode::from(&first_root.head),
                ComparisonNode::new(&first_tx.full_trace.trace[0], 0, 8)
            );

            assert_eq!(
                ComparisonNode::from(&first_root.head.inner[0]),
                ComparisonNode::new(&first_tx.full_trace.trace[1], 1, 1)
            );

            assert_eq!(
                ComparisonNode::from(&first_root.head.inner[0].inner[0]),
                ComparisonNode::new(&first_tx.full_trace.trace[2], 2, 0)
            );

            assert_eq!(
                ComparisonNode::from(&first_root.head.inner[1]),
                ComparisonNode::new(&first_tx.full_trace.trace[3], 3, 0)
            );

            assert_eq!(
                ComparisonNode::from(&first_root.head.inner[2]),
                ComparisonNode::new(&first_tx.full_trace.trace[4], 4, 0)
            );

            assert_eq!(
                ComparisonNode::from(&first_root.head.inner[3]),
                ComparisonNode::new(&first_tx.full_trace.trace[5], 5, 0)
            );

            assert_eq!(
                ComparisonNode::from(&first_root.head.inner[4]),
                ComparisonNode::new(&first_tx.full_trace.trace[6], 6, 0)
            );

            assert_eq!(
                ComparisonNode::from(&first_root.head.inner[5]),
                ComparisonNode::new(&first_tx.full_trace.trace[7], 7, 3)
            );

            assert_eq!(
                ComparisonNode::from(&first_root.head.inner[5].inner[0]),
                ComparisonNode::new(&first_tx.full_trace.trace[8], 8, 0)
            );

            assert_eq!(
                ComparisonNode::from(&first_root.head.inner[5].inner[1]),
                ComparisonNode::new(&first_tx.full_trace.trace[9], 9, 0)
            );

            assert_eq!(
                ComparisonNode::from(&first_root.head.inner[5].inner[2]),
                ComparisonNode::new(&first_tx.full_trace.trace[10], 10, 3)
            );

            assert_eq!(
                ComparisonNode::from(&first_root.head.inner[5].inner[2].inner[0]),
                ComparisonNode::new(&first_tx.full_trace.trace[11], 11, 0)
            );

            assert_eq!(
                ComparisonNode::from(&first_root.head.inner[5].inner[2].inner[1]),
                ComparisonNode::new(&first_tx.full_trace.trace[12], 12, 0)
            );

            assert_eq!(
                ComparisonNode::from(&first_root.head.inner[5].inner[2].inner[2]),
                ComparisonNode::new(&first_tx.full_trace.trace[13], 13, 0)
            );

            assert_eq!(
                ComparisonNode::from(&first_root.head.inner[6]),
                ComparisonNode::new(&first_tx.full_trace.trace[14], 14, 0)
            );

            assert_eq!(
                ComparisonNode::from(&first_root.head.inner[7]),
                ComparisonNode::new(&first_tx.full_trace.trace[15], 15, 0)
            );

        */
    }
}

*/
