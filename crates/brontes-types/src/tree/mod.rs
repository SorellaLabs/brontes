use std::collections::HashMap;

use rayon::prelude::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator};
use reth_primitives::{Address, Header, B256};
use serde::{Deserialize, Serialize};
use statrs::statistics::Statistics;
use tracing::error;
pub mod node;
pub mod root;
pub mod tx_info;
pub use node::*;
pub use root::*;
pub use tx_info::*;

use crate::{db::metadata::MetadataNoDex, normalized_actions::NormalizedAction};

#[derive(Debug, Serialize, Deserialize)]
pub struct BlockTree<V: NormalizedAction> {
    pub tx_roots:             Vec<Root<V>>,
    pub header:               Header,
    pub priority_fee_std_dev: f64,
    pub avg_priority_fee:     f64,
}

impl<V: NormalizedAction> BlockTree<V> {
    pub fn new(header: Header, tx_num: usize) -> Self {
        Self {
            tx_roots: Vec::with_capacity(tx_num),
            header,
            priority_fee_std_dev: 0.0,
            avg_priority_fee: 0.0,
        }
    }

    pub fn get_tx_info(&self, tx_hash: B256) -> Option<TxInfo> {
        self.tx_roots
            .par_iter()
            .find_any(|r| r.tx_hash == tx_hash)
            .map(|root| root.get_tx_info(self.header.number))
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

    pub fn get_prev_tx(&self, hash: B256) -> Option<B256> {
        let index = self.tx_roots.iter().position(|h| h.tx_hash == hash)?;
    
        if index == 0 {
            None
        } else {
            Some(self.tx_roots[index - 1].tx_hash)
        }
    }

    pub fn insert_root(&mut self, root: Root<V>) {
        self.tx_roots.push(root);
    }

    pub fn finalize_tree(&mut self) {
        // in case the block is empty
        if self.tx_roots.is_empty() {
            error!(block = self.header.number, "The block tree is empty");
            self.tx_roots.iter_mut().for_each(|root| root.finalize());
            return
        }

        // Initialize accumulator for total priority fee and vector of priority fees
        let mut total_priority_fee: f64 = 0.0;
        let mut priority_fees: Vec<f64> = Vec::new();

        for tx in &mut self.tx_roots {
            let priority_fee = (tx.gas_details.effective_gas_price
                - self.header.base_fee_per_gas.unwrap() as u128)
                as f64;
            priority_fees.push(priority_fee);
            total_priority_fee += priority_fee;

            tx.finalize();
        }

        self.avg_priority_fee = total_priority_fee / self.tx_roots.len() as f64;
        let std_dev = priority_fees.population_std_dev();
        self.priority_fee_std_dev = std_dev;
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

    pub fn modify_spans<T, F>(&mut self, find: T, modify: F)
    where
        T: Fn(&Node<V>) -> bool + Send + Sync,
        F: Fn(Vec<&mut Node<V>>) + Send + Sync,
    {
        self.tx_roots.par_iter_mut().for_each(|root| {
            root.modify_spans(&find, &modify);
        });
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

    //TODO: (Will) Write the docs for this
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

    pub fn label_private_txes(&mut self, metadata: &MetadataNoDex) {
        self.tx_roots
            .par_iter_mut()
            .for_each(|root| root.label_private_tx(metadata));
    }
}
