use std::{panic::AssertUnwindSafe, sync::Arc};

use alloy_consensus::Header;
use alloy_primitives::B256;
use itertools::Itertools;
use statrs::statistics::Statistics;
use tracing::{error, info, span, Level};

use crate::{normalized_actions::MultiCallFrameClassification, tree::types::NodeWithDataRef};

pub mod frontend_prunes;
pub use frontend_prunes::*;

use crate::db::traits::LibmdbxReader;
pub mod node;
mod types;
#[allow(unused_parens)]
pub mod util;
pub use util::*;
pub mod root;
pub mod tx_info;
pub use node::*;
pub use root::*;
pub use tx_info::*;
pub mod search_args;
pub use search_args::*;

use crate::{db::metadata::Metadata, normalized_actions::NormalizedAction};

type SpansAll<V> = TreeIterator<V, std::vec::IntoIter<(B256, Vec<Vec<V>>)>>;
type ClassifyData<V> = Option<(usize, Vec<MultiCallFrameClassification<V>>)>;

#[derive(Debug, Clone)]
pub struct BlockTree<V: NormalizedAction> {
    pub tx_roots: Vec<Root<V>>,
    pub header: Header,
    pub priority_fee_std_dev: f64,
    pub avg_priority_fee: f64,
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

    pub fn tx_must_contain_action(&self, tx_hash: B256, f: impl Fn(&V) -> bool) -> Option<bool> {
        self.tx_roots
            .iter()
            .find(|r| r.tx_hash == tx_hash)
            .map(|root| root.tx_must_contain_action(f))
    }

    pub fn get_tx_info_batch<DB: LibmdbxReader>(
        &self,
        tx_hash: &[B256],
        database: &DB,
    ) -> Vec<Option<TxInfo>> {
        let (roots, mut eoa_info_addr, mut contract_info_addr): (Vec<_>, Vec<_>, Vec<_>) = self
            .tx_roots
            .iter()
            .filter(|r| tx_hash.contains(&r.tx_hash))
            .map(|root| (root, root.head.address, root.get_to_address()))
            .multiunzip();

        // reduce db calls
        eoa_info_addr.sort_unstable();
        eoa_info_addr.dedup();

        contract_info_addr.sort_unstable();
        contract_info_addr.dedup();

        let Ok(contract) = database.try_fetch_searcher_contract_infos(contract_info_addr.clone())
        else {
            return vec![];
        };

        let Ok(address_meta) = database.try_fetch_address_metadatas(contract_info_addr) else {
            return vec![];
        };

        let Ok(eoa) = database.try_fetch_searcher_eoa_infos(eoa_info_addr) else { return vec![] };

        roots
            .into_iter()
            .map(|root| {
                root.get_tx_info_batch(self.header.number, &eoa, &contract, &address_meta)
                    .ok()
            })
            .collect()
    }

    pub fn get_tx_info<DB: LibmdbxReader>(&self, tx_hash: B256, database: &DB) -> Option<TxInfo> {
        self.tx_roots
            .iter()
            .find(|r| r.tx_hash == tx_hash)
            .and_then(|root| {
                root.get_tx_info(self.header.number, database)
                    .map_err(|e| error!(block=%self.header.number,"Database Error: {}", e ))
                    .ok()
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

    pub fn roots(&self) -> &[Root<V>] {
        &self.tx_roots
    }

    pub fn finalize_tree(&mut self) {
        self.run_in_span_mut(|this| {
            // in case the block is empty
            if this.tx_roots.is_empty() {
                info!(block = this.header.number, "The block tree is empty");
                this.tx_roots.iter_mut().for_each(|root| root.finalize());
                return;
            }

            // Initialize accumulator for total priority fee and vector of priority fees
            let mut total_priority_fee: f64 = 0.0;
            let mut priority_fees: Vec<f64> = Vec::new();

            for tx in &mut this.tx_roots {
                let priority_fee = (tx.gas_details.effective_gas_price
                    - this.header.base_fee_per_gas.unwrap_or_default() as u128)
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

    /// Collects subsets of actions that match the action criteria specified
    /// by the closure. This is useful for collecting the subtrees of a
    /// transaction that contain the wanted actions.
    pub fn collect_spans(
        self: Arc<Self>,
        hash: B256,
        call: TreeSearchBuilder<V>,
    ) -> TreeIterator<V, std::vec::IntoIter<Vec<V>>> {
        self.run_in_span_ref(|this| {
            if let Some(root) = this.tx_roots.iter().find(|r| r.tx_hash == hash) {
                TreeIterator::new(this.clone(), root.collect_spans(&call).into_iter())
            } else {
                TreeIterator::new(this.clone(), vec![].into_iter())
            }
        })
    }

    /// Collects all subsets of actions that match the action criteria specified
    /// by the closure. This is useful for collecting the subtrees of a
    /// transaction that contain the wanted actions.
    pub fn collect_spans_all(self: Arc<Self>, call: TreeSearchBuilder<V>) -> SpansAll<V> {
        self.run_in_span_ref(|this| {
            TreeIterator::new(
                this.clone(),
                this.tx_roots
                    .iter()
                    .map(|r| (r.tx_hash, r.collect_spans(&call)))
                    .collect::<Vec<(_, _)>>()
                    .into_iter(),
            )
        })
    }

    /// Collects all spans defined by the Search Args, then will allow
    /// modifications of the nodes found in the spans.
    pub fn modify_spans<F>(&mut self, find: TreeSearchBuilder<V>, modify: F)
    where
        F: Fn(Vec<&mut Node>, &mut NodeData<V>) + Send + Sync,
    {
        self.run_in_span_mut(|this| {
            this.tx_roots.iter_mut().for_each(|root| {
                root.modify_spans(&find, &modify);
            });
        })
    }

    /// For the given tx hash, goes through the tree and collects all actions
    /// specified by the tree search builder.
    pub fn collect(
        self: Arc<Self>,
        hash: &B256,
        call: TreeSearchBuilder<V>,
    ) -> TreeIterator<V, std::vec::IntoIter<V>> {
        self.run_in_span_ref(|this| {
            if let Some(root) = this.tx_roots.iter().find(|r| r.tx_hash == *hash) {
                TreeIterator::new(this.clone(), root.collect(&call).into_iter())
            } else {
                TreeIterator::new(this.clone(), vec![].into_iter())
            }
        })
    }

    /// For all transactions, goes through the tree and collects all actions
    /// specified by the tree search builder.
    pub fn collect_all(
        self: Arc<Self>,
        call: TreeSearchBuilder<V>,
    ) -> TreeIterator<V, std::vec::IntoIter<(B256, Vec<V>)>> {
        self.run_in_span_ref(|this| {
            TreeIterator::new(
                this.clone(),
                this.tx_roots
                    .iter()
                    .map(|r| (r.tx_hash, r.collect(&call)))
                    .collect::<Vec<(_, _)>>()
                    .into_iter(),
            )
        })
    }

    pub fn collect_txes(
        self: Arc<Self>,
        txes: &[B256],
        call: TreeSearchBuilder<V>,
    ) -> TreeIterator<V, std::vec::IntoIter<Vec<V>>> {
        self.run_in_span_ref(|this| {
            TreeIterator::new(
                this.clone(),
                txes.iter()
                    .map(|tx| this.clone().collect(tx, call.clone()).collect_vec())
                    .collect::<Vec<_>>()
                    .into_iter(),
            )
        })
    }

    /// Takes Vec<(TransactionIndex, `Vec<ActionIndex>`)>
    /// for every action index of a transaction index, This function grabs all
    /// child nodes of the action index if and only if they are specified in
    /// the classification function of the action index node.
    pub fn collect_and_classify(&mut self, search_params: &[ClassifyData<V>]) {
        self.run_in_span_mut(|this| {
            this.tx_roots
                .iter_mut()
                .zip(search_params.iter())
                .filter_map(|(root, opt)| Some((root, opt.as_ref()?)))
                .for_each(|(root, (_, subtraces))| {
                    root.collect_child_traces_and_classify(subtraces);
                });
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
            this.tx_roots
                .iter_mut()
                .for_each(|r| r.modify_node_if_contains_childs(&find, &modify));
        })
    }

    pub fn label_private_txes(&mut self, metadata: &Metadata) {
        self.tx_roots
            .iter_mut()
            .for_each(|root| root.label_private_tx(metadata));
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
        WantedData: Fn(NodeWithDataRef<'_, V>) -> R + Sync,
        ClassifyRemovalIndex: Fn(&Vec<R>, &Node, &NodeData<V>) -> Vec<u64> + Sync,
    {
        self.run_in_span_mut(|this| {
            this.tx_roots.iter_mut().for_each(|root| {
                root.remove_duplicate_data(&find, &classify, &info, &find_removal)
            });
        });
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

    fn run_in_span_ref<Ret: Send>(self: Arc<Self>, action: impl Fn(Arc<Self>) -> Ret) -> Ret {
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
    use std::sync::Arc;

    use alloy_primitives::hex;
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{normalized_actions::Action, BlockTree, TreeSearchBuilder};

    async fn load_tree() -> Arc<BlockTree<Action>> {
        let classifier_utils = ClassifierTestUtils::new().await;
        let tx = hex!("31dedbae6a8e44ec25f660b3cd0e04524c6476a0431ab610bb4096f82271831b").into();
        classifier_utils.build_tree_tx(tx).await.unwrap().into()
    }

    #[brontes_macros::test]
    async fn test_collect() {
        let tx = &hex!("31dedbae6a8e44ec25f660b3cd0e04524c6476a0431ab610bb4096f82271831b").into();
        let tree = load_tree().await;

        let burns = tree
            .clone()
            .collect(tx, TreeSearchBuilder::default().with_action(Action::is_burn))
            .collect::<Vec<_>>();
        assert_eq!(burns.len(), 1);
        let swaps = tree
            .collect(tx, TreeSearchBuilder::default().with_action(Action::is_swap))
            .collect::<Vec<_>>();
        assert_eq!(swaps.len(), 3);
    }

    #[brontes_macros::test]
    async fn test_collect_spans() {
        let tx = hex!("31dedbae6a8e44ec25f660b3cd0e04524c6476a0431ab610bb4096f82271831b").into();
        let tree = load_tree().await;
        let spans = tree
            .collect_spans(
                tx,
                TreeSearchBuilder::default()
                    .with_actions([])
                    .child_nodes_contain([Action::is_transfer, Action::is_swap]),
            )
            .collect::<Vec<_>>();

        assert!(!spans.is_empty());
        assert_eq!(spans.len(), 4);
    }

    #[brontes_macros::test]
    async fn test_collect_and_classify() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let tx = hex!("f9e7365f9c9c2859effebe61d5d19f44dcbf4d2412e7bcc5c511b3b8fbfb8b8d").into();
        let tree = Arc::new(classifier_utils.build_tree_tx(tx).await.unwrap());
        let mut actions = tree
            .collect(&tx, TreeSearchBuilder::default().with_action(Action::is_batch))
            .collect::<Vec<_>>();
        assert!(!actions.is_empty());
        let action = actions.remove(0);

        let Action::Batch(b) = action else {
            panic!("not batch");
        };

        assert!(
            b.user_swaps.iter().all(|swap| swap.trace_index != 0),
            "batch user swaps wasn't set"
        );
    }
}
