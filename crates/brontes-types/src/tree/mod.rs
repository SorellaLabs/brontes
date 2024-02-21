use std::{collections::HashMap, panic::AssertUnwindSafe};

use rayon::{
    prelude::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator},
    ThreadPool, ThreadPoolBuilder,
};
use reth_primitives::{Header, B256};
use statrs::statistics::Statistics;
use tracing::{error, span, Level};

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
    pub tx_roots:             Vec<Root<V>>,
    pub header:               Header,
    pub priority_fee_std_dev: f64,
    pub avg_priority_fee:     f64,
    pub tp:                   ThreadPool,
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
                        .map_err(|e| error!(block=%self.header.number,"Database Error: {}", e ))
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
        self.run_in_span_mut(|this| {
            // in case the block is empty
            if this.tx_roots.is_empty() {
                error!(block = this.header.number, "The block tree is empty");
                this.tx_roots.iter_mut().for_each(|root| root.finalize());
                return
            }

            // Initialize accumulator for total priority fee and vector of priority fees
            let mut total_priority_fee: f64 = 0.0;
            let mut priority_fees: Vec<f64> = Vec::new();

            for tx in &mut this.tx_roots {
                let priority_fee = (tx.gas_details.effective_gas_price
                    - this.header.base_fee_per_gas.unwrap() as u128)
                    as f64;
                priority_fees.push(priority_fee);
                total_priority_fee += priority_fee;

                tx.finalize();
            }

            this.avg_priority_fee = total_priority_fee / this.tx_roots.len() as f64;
            let std_dev = priority_fees.population_std_dev();
            this.priority_fee_std_dev = std_dev;
        })
    }

    pub fn get_hashes(&self) -> Vec<B256> {
        self.tx_roots.iter().map(|r| r.tx_hash).collect()
    }

    /// Collects all subsets of actions that match the action criteria specified
    /// by the closure. This is useful for collecting the subtrees of a
    /// transaction that contain the wanted actions.
    pub fn collect_spans(&self, hash: B256, call: TreeSearchBuilder<V>) -> Vec<Vec<V>> {
        self.run_in_span_ref(|this| {
            if let Some(root) = this.tx_roots.iter().find(|r| r.tx_hash == hash) {
                root.collect_spans(&call)
            } else {
                vec![]
            }
        })
    }

    /// Collects all spans defined by the Search Args, then will allow
    /// modifications of the nodes found in the spans.
    pub fn modify_spans<F>(&mut self, find: TreeSearchBuilder<V>, modify: F)
    where
        F: Fn(Vec<&mut Node>, &mut NodeData<V>) + Send + Sync,
    {
        self.run_in_span_mut(|this| {
            this.tp.install(|| {
                this.tx_roots.par_iter_mut().for_each(|root| {
                    root.modify_spans(&find, &modify);
                });
            });
        })
    }

    /// For the given tx hash, goes through the tree and collects all actions
    /// specified by the tree search builder.
    pub fn collect(&self, hash: B256, call: TreeSearchBuilder<V>) -> Vec<V> {
        self.run_in_span_ref(|this| {
            if let Some(root) = this.tx_roots.iter().find(|r| r.tx_hash == hash) {
                root.collect(&call)
            } else {
                vec![]
            }
        })
    }

    /// For all transactions, goes through the tree and collects all actions
    /// specified by the tree search builder.
    pub fn collect_all(&self, call: TreeSearchBuilder<V>) -> HashMap<B256, Vec<V>> {
        self.run_in_span_ref(|this| {
            this.tp.install(|| {
                this.tx_roots
                    .par_iter()
                    .map(|r| (r.tx_hash, r.collect(&call)))
                    .collect()
            })
        })
    }

    /// Takes Vec<(TransactionIndex, Vec<ActionIndex>)>
    /// for every action index of a transaction index, This function grabs all
    /// child nodes of the action index if and only if they are specified in
    /// the classification function of the action index node.
    pub fn collect_and_classify(&mut self, search_params: &[Option<(usize, Vec<u64>)>]) {
        self.run_in_span_mut(|this| {
            let mut roots_with_search_params = this
                .tx_roots
                .iter_mut()
                .zip(search_params.iter())
                .collect::<Vec<_>>();

            this.tp.install(|| {
                roots_with_search_params
                    .par_iter_mut()
                    .filter_map(|(root, opt)| Some((root, opt.as_ref()?)))
                    .for_each(|(root, (_, subtraces))| {
                        root.collect_child_traces_and_classify(subtraces);
                    });
            });
        })
    }

    /// Collects all subsets of actions that match the action criteria specified
    /// by the closure. This is useful for collecting the subtrees of a
    /// transaction that contain the wanted actions.
    pub fn collect_spans_all(&self, call: TreeSearchBuilder<V>) -> HashMap<B256, Vec<Vec<V>>> {
        self.run_in_span_ref(|this| {
            this.tp.install(|| {
                this.tx_roots
                    .par_iter()
                    .map(|r| (r.tx_hash, r.collect_spans(&call)))
                    .collect()
            })
        })
    }

    /// Uses the search args to find a given nodes. Specifically if a node has
    /// childs that the search args define. Then calls the modify function
    /// on the current node.
    pub fn modify_node_if_contains_childs<F>(&mut self, find: TreeSearchBuilder<V>, modify: F)
    where
        F: Fn(&mut Node, &mut NodeData<V>) + Send + Sync,
    {
        self.run_in_span_mut(|this| {
            this.tp.install(|| {
                this.tx_roots
                    .par_iter_mut()
                    .for_each(|r| r.modify_node_if_contains_childs(&find, &modify));
            })
        })
    }

    /// Uses search args to collect two types of nodes. Nodes that could be a
    /// parent to a child node that we want to remove. and child nodes we
    /// want to remove. These are both collected and passed to the classifiy
    /// removal index function. This function will allow the user to look at
    /// all of the parent nodes and possible removal nodes and return the
    /// index of nodes that will be removed from the tree.
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
        self.run_in_span_mut(|this| {
            this.tp.install(|| {
                this.tx_roots.par_iter_mut().for_each(|root| {
                    root.remove_duplicate_data(&find, &classify, &info, &find_removal)
                });
            })
        })
    }

    pub fn label_private_txes(&mut self, metadata: &Metadata) {
        self.tx_roots
            .iter_mut()
            .for_each(|root| root.label_private_tx(metadata));
    }

    /// catches all panics and errors and makes sure to log with block number to
    /// ensure easy debugging
    fn run_in_span_mut<Ret: Send>(&mut self, action: impl Fn(&mut Self) -> Ret) -> Ret {
        let span = span!(Level::ERROR, "brontes-tree", block = self.header.number);
        let g = span.enter();

        let res = std::panic::catch_unwind(AssertUnwindSafe(|| action(self)));

        let res = match res {
            Ok(r) => r,
            Err(e) => {
                let error = e.downcast_ref::<String>().cloned().unwrap_or(
                    e.downcast_ref::<&str>()
                        .map(|s| (*s).to_string())
                        .unwrap_or_default(),
                );
                tracing::error!(error=%error, "hit panic on a tree action, exiting");
                panic!("{e:?}");
            }
        };
        drop(g);

        res
    }

    fn run_in_span_ref<Ret: Send>(&self, action: impl Fn(&Self) -> Ret) -> Ret {
        let span = span!(Level::ERROR, "brontes-tree", block = self.header.number);
        let g = span.enter();

        let res = std::panic::catch_unwind(AssertUnwindSafe(|| action(self)));

        let res = match res {
            Ok(r) => r,
            Err(e) => {
                let error = e.downcast_ref::<String>().cloned().unwrap_or(
                    e.downcast_ref::<&str>()
                        .map(|s| (*s).to_string())
                        .unwrap_or_default(),
                );
                tracing::error!(error=%error, "hit panic on a tree action, exiting");
                panic!("{e:?}");
            }
        };
        drop(g);

        res
    }
}

#[cfg(test)]
pub mod test {
    use alloy_primitives::hex;
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{normalized_actions::Actions, BlockTree, TreeSearchBuilder};

    async fn load_tree() -> BlockTree<Actions> {
        let classifier_utils = ClassifierTestUtils::new().await;
        let tx = hex!("31dedbae6a8e44ec25f660b3cd0e04524c6476a0431ab610bb4096f82271831b").into();
        classifier_utils.build_tree_tx(tx).await.unwrap()
    }

    #[brontes_macros::test]
    async fn test_collect() {
        let tx = hex!("31dedbae6a8e44ec25f660b3cd0e04524c6476a0431ab610bb4096f82271831b").into();
        let tree: BlockTree<Actions> = load_tree().await;

        let burns = tree.collect(tx, TreeSearchBuilder::default().with_action(Actions::is_burn));
        assert_eq!(burns.len(), 1);
        let swaps = tree.collect(tx, TreeSearchBuilder::default().with_action(Actions::is_swap));
        assert_eq!(swaps.len(), 3);
    }

    #[brontes_macros::test]
    async fn test_collect_spans() {
        let tx = hex!("31dedbae6a8e44ec25f660b3cd0e04524c6476a0431ab610bb4096f82271831b").into();
        let tree: BlockTree<Actions> = load_tree().await;
        let spans = tree.collect_spans(
            tx,
            TreeSearchBuilder::default()
                .with_actions([])
                .child_nodes_contain([Actions::is_transfer, Actions::is_swap]),
        );

        assert!(!spans.is_empty());
        assert_eq!(spans.len(), 4);
    }

    #[brontes_macros::test]
    async fn test_remove_duplicate_data() {
        let mut tree: BlockTree<Actions> = load_tree().await;

        let pre_transfers = tree
            .collect_all(TreeSearchBuilder::default().with_action(Actions::is_transfer))
            .into_values()
            .flatten()
            .collect::<Vec<_>>();

        tree.remove_duplicate_data(
            TreeSearchBuilder::default().with_action(Actions::is_swap),
            TreeSearchBuilder::default().with_action(Actions::is_transfer),
            |node, data| (node.index, data.get_ref(node.data).cloned()),
            |other_nodes, node, data| {
                let Some(swap_data) = data.get_ref(node.data) else {
                    return vec![];
                };
                let swap_data = swap_data.force_swap_ref();

                other_nodes
                    .iter()
                    .filter_map(|(index, data)| {
                        let Actions::Transfer(transfer) = data.as_ref()? else {
                            return None;
                        };
                        if (transfer.amount == swap_data.amount_in
                            || (&transfer.amount + &transfer.fee) == swap_data.amount_out)
                            && (transfer.to == swap_data.pool || transfer.from == swap_data.pool)
                        {
                            return Some(*index)
                        }
                        None
                    })
                    .collect::<Vec<_>>()
            },
        );
        let post_transfers = tree
            .collect_all(TreeSearchBuilder::default().with_action(Actions::is_transfer))
            .into_values()
            .flatten()
            .collect::<Vec<_>>();

        assert!(pre_transfers.len() > post_transfers.len());
    }

    #[brontes_macros::test]
    async fn test_collect_and_classify() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let tx = hex!("f9e7365f9c9c2859effebe61d5d19f44dcbf4d2412e7bcc5c511b3b8fbfb8b8d").into();
        let tree = classifier_utils.build_tree_tx(tx).await.unwrap();
        let mut actions =
            tree.collect(tx, TreeSearchBuilder::default().with_action(Actions::is_batch));
        assert!(!actions.is_empty());
        let action = actions.remove(0);

        let Actions::Batch(b) = action else {
            panic!("not batch");
        };

        assert!(
            b.user_swaps
                .iter()
                .map(|swap| swap.trace_index != 0)
                .all(|t| t),
            "batch user swaps wasn't set"
        );
    }
}
