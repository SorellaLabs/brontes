use std::ops::Deref;

use alloy_primitives::TxHash;
use brontes_core::{
    decoding::TracingProvider, BlockTracesWithHeaderAnd, TraceLoader, TraceLoaderError,
    TxTracesWithHeaderAnd,
};
use brontes_pricing::types::DexPriceMsg;
use brontes_types::tree::BlockTree;
use futures::future::join_all;
use thiserror::Error;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

use crate::{Actions, Classifier};

/// Classifier specific functionality
pub struct ClassifierTestUtils {
    trace_loader: TraceLoader,
    classifier:   Classifier<'static, Box<dyn TracingProvider>>,

    dex_pricing_receiver: UnboundedReceiver<DexPriceMsg>,
}

impl ClassifierTestUtils {
    pub fn new() -> Self {
        let trace_loader = TraceLoader::new();
        let (tx, rx) = unbounded_channel();
        let classifier = Classifier::new(trace_loader.libmdbx, tx, trace_loader.get_provider());

        Self { classifier, trace_loader, dex_pricing_receiver: rx }
    }

    pub fn get_pricing_receiver(&mut self) -> &mut UnboundedReceiver<DexPriceMsg> {
        &mut self.dex_pricing_receiver
    }

    pub async fn build_raw_tree_tx(
        &self,
        tx_hash: TxHash,
    ) -> Result<BlockTree<Actions>, ClassifierTestUtilsError> {
        let TxTracesWithHeaderAnd { trace, header, .. } =
            self.trace_loader.get_tx_trace_with_header(tx_hash).await?;

        let tx_roots = self
            .classifier
            .build_all_tx_trees(vec![trace], &header)
            .await;

        let mut tree = BlockTree::new(header, tx_roots.len());

        tx_roots.into_iter().for_each(|root_data| {
            tree.insert_root(root_data.root);
        });

        Ok(tree)
    }

    pub async fn build_raw_tree_txes(
        &self,
        tx_hashes: Vec<TxHash>,
    ) -> Result<Vec<BlockTree<Actions>>, ClassifierTestUtilsError> {
        Ok(join_all(
            self.trace_loader
                .get_tx_traces_with_header(tx_hashes)
                .await?
                .into_iter()
                .map(|data| async move {
                    let tx_roots = self
                        .classifier
                        .build_all_tx_trees(data.traces, &data.header)
                        .await;

                    let mut tree = BlockTree::new(data.header, tx_roots.len());

                    tx_roots.into_iter().for_each(|root_data| {
                        tree.insert_root(root_data.root);
                    });

                    tree
                }),
        )
        .await)
    }

    pub async fn build_tree_tx(
        &self,
        tx_hash: TxHash,
    ) -> Result<BlockTree<Actions>, ClassifierTestUtilsError> {
        let TxTracesWithHeaderAnd { trace, header, .. } =
            self.trace_loader.get_tx_trace_with_header(tx_hash).await?;
        let (_, tree) = self.classifier.build_block_tree(vec![trace], header).await;

        Ok(tree)
    }

    pub async fn build_tree_txes(
        &self,
        tx_hashes: Vec<TxHash>,
    ) -> Result<Vec<BlockTree<Actions>>, ClassifierTestUtilsError> {
        Ok(join_all(
            self.trace_loader
                .get_tx_traces_with_header(tx_hashes)
                .await?
                .into_iter()
                .map(|block_info| async move {
                    let (_, tree) = self
                        .classifier
                        .build_block_tree(block_info.traces, block_info.header)
                        .await;
                    tree
                }),
        )
        .await)
    }

    pub async fn build_tree_block(
        &self,
        block: u64,
    ) -> Result<BlockTree<Actions>, ClassifierTestUtilsError> {
        let BlockTracesWithHeaderAnd { traces, header, .. } = self
            .trace_loader
            .get_block_traces_with_header(block)
            .await?;
        let (_, tree) = self.classifier.build_block_tree(traces, header).await;

        Ok(tree)
    }
}

impl Deref for ClassifierTestUtils {
    type Target = TraceLoader;

    fn deref(&self) -> &Self::Target {
        &self.trace_loader
    }
}

#[derive(Debug, Error)]
pub enum ClassifierTestUtilsError {
    #[error(transparent)]
    TraceLoaderError(#[from] TraceLoaderError),
}
