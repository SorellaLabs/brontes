use crate::normalized_actions::NormalizedAction;
use malachite::Rational;
use rayon::prelude::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator};
use reth_primitives::{Address, Header, H256, U256};
use std::collections::HashMap;
use tracing::error;
pub struct Node<V: NormalizedAction> {
    pub inner: Vec<Node<V>>,
    pub frozen: bool,

    /// This only has values when the node is frozen
    pub subactions: Vec<V>,
    pub address: Address,
    pub data: V,
}

impl<V: NormalizedAction> Node<V> {
    pub fn is_root(&self) -> bool {
        self.frozen
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen
    }

    pub fn freeze(&mut self) {
        self.subactions = self.get_all_sub_actions();
        self.frozen = true;

        self.inner.iter_mut().for_each(|f| f.freeze());
    }

    /// The address here is the from address for the trace
    pub fn insert(&mut self, address: Address, n: Node<V>) -> bool {
        if self.frozen {
            return false
        }

        if address == self.address {
            let mut cur_stack = self.current_call_stack();
            cur_stack.pop();
            if !cur_stack.contains(&address) {
                self.inner.push(n);
                return true
            }
        }

        let last = self.inner.last_mut().expect("building tree went wrong");
        last.insert(address, n)
    }

    pub fn get_all_sub_actions(&self) -> Vec<V> {
        if self.frozen {
            self.subactions.clone()
        } else {
            self.inner.iter().flat_map(|inner| inner.get_all_sub_actions()).collect()
        }
    }

    pub fn all_sub_addresses(&self) -> Vec<Address> {
        self.inner.iter().flat_map(|i| i.all_sub_addresses()).chain(vec![self.address]).collect()
    }

    pub fn current_call_stack(&self) -> Vec<Address> {
        let Some(mut stack) = self.inner.last().map(|n| n.current_call_stack()) else {
            return vec![self.address]
        };

        stack.push(self.address);

        stack
    }

    pub fn inspect<F>(&self, result: &mut Vec<Vec<V>>, call: &F) -> bool
    where
        F: Fn(&Node<V>) -> bool,
    {
        // the previous sub-action was best
        if !call(self) {
            return false
        }
        let lower_has_better = self.inner.iter().map(|i| i.inspect(result, call)).any(|f| f);

        // if all child nodes don't have a best sub-action. Then the current node is the best.
        if !lower_has_better {
            result.push(self.get_all_sub_actions());
            return true
        }
        // lower node has a better sub-action.
        false
    }

    pub fn dyn_classify<T, F>(
        &mut self,
        find: &T,
        call: &F,
        result: &mut Vec<(Address, (Address, Address))>,
    ) -> bool
    where
        T: Fn(Address, Vec<V>) -> bool,
        F: Fn(&mut Node<V>) -> Option<(Address, (Address, Address))> + Send + Sync,
    {
        let works = find(self.address, self.get_all_sub_actions());
        if !works {
            return false
        }

        let lower_has_better = self.inner.iter_mut().any(|i| i.dyn_classify(find, call, result));

        if !lower_has_better {
            if let Some(res) = call(self) {
                result.push(res);
            }
        }
        true
    }
}

pub struct Root<V: NormalizedAction> {
    pub head: Node<V>,
    pub tx_hash: H256,
    pub tx_index: usize,
    pub private: bool,
}

impl<V: NormalizedAction> Root<V> {
    pub fn insert(&mut self, from: Address, node: Node<V>) {
        if !self.head.insert(from, node) {
            error!("failed to insert node");
        }
    }

    pub fn inspect<F>(&self, call: &F) -> Vec<Vec<V>>
    where
        F: Fn(&Node<V>) -> bool,
    {
        let mut result = Vec::new();
        self.head.inspect(&mut result, call);

        result
    }

    pub fn dyn_classify<T, F>(&mut self, find: &T, call: &F) -> Vec<(Address, (Address, Address))>
    where
        T: Fn(Address, Vec<V>) -> bool,
        F: Fn(&mut Node<V>) -> Option<(Address, (Address, Address))> + Send + Sync,
    {
        // bool is used for recursion
        let mut results = Vec::new();
        let _ = self.head.dyn_classify(find, call, &mut results);

        results
    }

    pub fn freeze(&mut self) {
        self.head.freeze();
    }
}

pub struct TimeTree<V: NormalizedAction> {
    pub roots: Vec<Root<V>>,
    pub header: Header,
    eth_prices: (Rational, Rational),
}

impl<V: NormalizedAction> TimeTree<V> {
    pub fn new(txes: usize, header: Header, eth_prices: Rational) -> Self {
        Self { roots: Vec::with_capacity(txes), header, eth_prices}
    }

    pub fn insert_root(&mut self, root: Root<V>) {
        self.roots.push(root);
    }

    pub fn freeze_tree(&mut self) {
        self.roots.iter_mut().for_each(|root| root.freeze());
    }

    pub fn insert_node(&mut self, from: Address, node: Node<V>) {
        self.roots.last_mut().expect("no root_nodes inserted").insert(from, node);
    }

    pub fn get_hashes(&self) -> Vec<H256> {
        self.roots.iter().map(|r| r.tx_hash).collect()
    }

    pub fn inspect<F>(&self, hash: H256, call: F) -> Vec<Vec<V>>
    where
        F: Fn(&Node<V>) -> bool,
    {
        if let Some(root) = self.roots.iter().find(|r| r.tx_hash == hash) {
            root.inspect(&call)
        } else {
            vec![]
        }
    }

    pub fn inspect_all<F>(&self, call: F) -> HashMap<H256, Vec<Vec<V>>>
    where
        F: Fn(&Node<V>) -> bool + Send + Sync,
    {
        self.roots.par_iter().map(|r| (r.tx_hash, r.inspect(&call))).collect()
    }

    /// the first function parses down through the tree to the point where we are at
    /// the lowest subset of the valid action. once we reach here, the call function gets
    /// executed in order to capture the data
    pub fn dyn_classify<T, F>(&mut self, find: T, call: F) -> Vec<(Address, (Address, Address))>
    where
        T: Fn(Address, Vec<V>) -> bool + Sync,
        F: Fn(&mut Node<V>) -> Option<(Address, (Address, Address))> + Send + Sync,
    {
        self.roots.par_iter_mut().flat_map(|root| root.dyn_classify(&find, &call)).collect()
    }
}
