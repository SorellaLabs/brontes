use std::collections::HashMap;

use rayon::{
    prelude::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator},
    ThreadPool, ThreadPoolBuilder,
};
use reth_primitives::{Header, B256};
use statrs::statistics::Statistics;
use tracing::error;

use crate::db::traits::LibmdbxReader;
pub mod node;
pub mod root;
pub mod tx_info;
pub use node::*;
pub use root::*;
pub use tx_info::*;
pub mod search_args;
pub use search_args::*;

use crate::{db::metadata::Metadata, normalized_actions::NormalizedAction};

const MAX_SEARCH_THREADS: usize = 4;

#[derive(Debug)]
pub struct BlockTree<V: NormalizedAction> {
    pub tx_roots: Vec<Root<V>>,
    pub header: Header,
    pub priority_fee_std_dev: f64,
    pub avg_priority_fee: f64,
    pub tp: ThreadPool,
}

impl<V: NormalizedAction> BlockTree<V> {
    pub fn new(header: Header, tx_num: usize) -> Self {
        Self {
            tx_roots: Vec::with_capacity(tx_num),
            header,
            priority_fee_std_dev: 0.0,
            avg_priority_fee: 0.0,
            tp: ThreadPoolBuilder::new()
                .num_threads(MAX_SEARCH_THREADS)
                .build()
                .unwrap(),
        }
    }

    pub fn get_tx_info<DB: LibmdbxReader>(&self, tx_hash: B256, database: &DB) -> Option<TxInfo> {
        self.tp.install(|| {
            self.tx_roots
                .par_iter()
                .find_any(|r| r.tx_hash == tx_hash)
                .and_then(|root| {
                    root.get_tx_info(self.header.number, database)
                        .map_err(|e| error!("Database Error: {}", e))
                        .ok()
                })
        })
    }

    pub fn get_root(&self, tx_hash: B256) -> Option<&Root<V>> {
        self.tx_roots.iter().find(|r| r.tx_hash == tx_hash)
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
            return;
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

    pub fn get_hashes(&self) -> Vec<B256> {
        self.tx_roots.iter().map(|r| r.tx_hash).collect()
    }

    /// Collects all subsets of actions that match the action criteria specified
    /// by the closure. This is useful for collecting the subtrees of a
    /// transaction that contain the wanted actions.
    pub fn collect_spans(&self, hash: B256, call: TreeSearchBuilder<V>) -> Vec<Vec<V>> {
        if let Some(root) = self.tx_roots.iter().find(|r| r.tx_hash == hash) {
            root.collect_spans(&call)
        } else {
            vec![]
        }
    }

    /// Collects all spans defined by the Search Args, then will allow modifications
    /// of the nodes found in the spans.
    pub fn modify_spans<F>(&mut self, find: TreeSearchBuilder<V>, modify: F)
    where
        F: Fn(Vec<&mut Node>, &mut NodeData<V>) + Send + Sync,
    {
        self.tp.install(|| {
            self.tx_roots.par_iter_mut().for_each(|root| {
                root.modify_spans(&find, &modify);
            });
        });
    }

    /// For the given tx hash, goes through the tree and collects all actions
    /// specified by the tree search builder.
    pub fn collect(&self, hash: B256, call: TreeSearchBuilder<V>) -> Vec<V> {
        if let Some(root) = self.tx_roots.iter().find(|r| r.tx_hash == hash) {
            root.collect(&call)
        } else {
            vec![]
        }
    }

    /// For all transactions, goes through the tree and collects all actions
    /// specified by the tree search builder.
    pub fn collect_all(&self, call: TreeSearchBuilder<V>) -> HashMap<B256, Vec<V>> {
        self.tp.install(|| {
            self.tx_roots
                .par_iter()
                .map(|r| (r.tx_hash, r.collect(&call)))
                .collect()
        })
    }

    /// Takes Vec<(TransactionIndex, Vec<ActionIndex>)>
    /// for every action index of a transaction index, This function grabs all
    /// child nodes of the action index if and only if they are specified in
    /// the classification function of the action index node.
    pub fn collect_and_classify(&mut self, search_params: &[Option<(usize, Vec<u64>)>]) {
        let mut roots_with_search_params = self
            .tx_roots
            .iter_mut()
            .zip(search_params.iter())
            .collect::<Vec<_>>();

        self.tp.install(|| {
            roots_with_search_params
                .par_iter_mut()
                .filter_map(|(root, opt)| Some((root, opt.as_ref()?)))
                .for_each(|(root, (_, subtraces))| {
                    root.collect_child_traces_and_classify(subtraces);
                });
        });
    }

    /// Collects all subsets of actions that match the action criteria specified
    /// by the closure. This is useful for collecting the subtrees of a
    /// transaction that contain the wanted actions.
    pub fn collect_spans_all(&self, call: TreeSearchBuilder<V>) -> HashMap<B256, Vec<Vec<V>>> {
        self.tp.install(|| {
            self.tx_roots
                .par_iter()
                .map(|r| (r.tx_hash, r.collect_spans(&call)))
                .collect()
        })
    }

    /// Uses the search args to find a given nodes. Specifically if a node has childs
    /// that the search args define. Then calls the modify function on the current node.
    pub fn modify_node_if_contains_childs<F>(&mut self, find: TreeSearchBuilder<V>, modify: F)
    where
        F: Fn(&mut Node, &mut NodeData<V>) + Send + Sync,
    {
        self.tp.install(|| {
            self.tx_roots
                .par_iter_mut()
                .for_each(|r| r.modify_node_if_contains_childs(&find, &modify));
        })
    }

    /// Uses search args to collect two types of nodes. Nodes that could be a parent to
    /// a child node that we want to remove. and child nodes we want to remove.
    /// These are both collected and passed to the classifiy removal index function.
    /// This function will allow the user to look at all of the parent nodes and possible removal
    /// nodes and return the index of nodes that will be removed from the tree.
    pub fn remove_duplicate_data<ClassifyRemovalIndex, WantedData, R>(
        &mut self,
        find: TreeSearchBuilder<V>,
        find_removal: TreeSearchBuilder<V>,
        info: WantedData,
        classify: ClassifyRemovalIndex,
    ) where
        WantedData: Fn(&Node, &NodeData<V>) -> R + Sync,
        ClassifyRemovalIndex: Fn(&Vec<R>, &Node, &NodeData<V>) -> Vec<u64> + Sync,
    {
        self.tp.install(|| {
            self.tx_roots.par_iter_mut().for_each(|root| {
                root.remove_duplicate_data(&find, &classify, &info, &find_removal)
            });
        })
    }

    pub fn label_private_txes(&mut self, metadata: &Metadata) {
        self.tx_roots
            .iter_mut()
            .for_each(|root| root.label_private_tx(metadata));
    }
}
