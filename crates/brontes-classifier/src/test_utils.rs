use std::{collections::HashMap, ops::Deref};

use alloy_primitives::{Address, TxHash};
use brontes_core::{
    decoding::TracingProvider, missing_decimals::load_missing_decimals, BlockTracesWithHeaderAnd,
    TraceLoader, TraceLoaderError, TxTracesWithHeaderAnd,
};
use brontes_database::libmdbx::{LibmdbxReadWriter, LibmdbxReader};
use brontes_pricing::{types::DexPriceMsg, BrontesBatchPricer, GraphManager};
use brontes_types::{
    db::dex::DexQuotes,
    tree::{BlockTree, Node},
};
use futures::{future::join_all, StreamExt};
use thiserror::Error;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

use crate::{Actions, Classifier};

/// Classifier specific functionality
pub struct ClassifierTestUtils {
    trace_loader: TraceLoader,
    classifier:   Classifier<'static, Box<dyn TracingProvider>, LibmdbxReadWriter>,

    dex_pricing_receiver: UnboundedReceiver<DexPriceMsg>,
}

impl ClassifierTestUtils {
    pub fn new() -> Self {
        let trace_loader = TraceLoader::new();
        let (tx, rx) = unbounded_channel();
        let classifier = Classifier::new(trace_loader.libmdbx, tx, trace_loader.get_provider());

        Self { classifier, trace_loader, dex_pricing_receiver: rx }
    }

    async fn init_dex_pricer(
        &self,
        block: u64,
        end_block: Option<u64>,
        quote_asset: Address,
        rx: UnboundedReceiver<DexPriceMsg>,
    ) -> Result<BrontesBatchPricer<Box<dyn TracingProvider>>, ClassifierTestUtilsError> {
        let pairs = self
            .libmdbx
            .protocols_created_before(block)
            .map_err(|_| ClassifierTestUtilsError::LibmdbxError)?;

        let pair_graph = GraphManager::init_from_db_state(
            pairs,
            HashMap::default(),
            Box::new(|_, _| None),
            Box::new(|_, _, _| {}),
        );

        let created_pools = if let Some(end_block) = end_block {
            self.libmdbx
                .protocols_created_range(block + 1, end_block)
                .unwrap()
                .into_iter()
                .flat_map(|(_, pools)| {
                    pools
                        .into_iter()
                        .map(|(addr, protocol, pair)| (addr, (protocol, pair)))
                        .collect::<Vec<_>>()
                })
                .collect::<HashMap<_, _>>()
        } else {
            HashMap::new()
        };
        Ok(BrontesBatchPricer::new(
            quote_asset,
            pair_graph,
            rx,
            self.get_provider(),
            block,
            created_pools,
        ))
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

    pub async fn build_tree_tx_with_pricing(
        &self,
        tx_hash: TxHash,
        quote_asset: Address,
    ) -> Result<(BlockTree<Actions>, DexQuotes), ClassifierTestUtilsError> {
        let TxTracesWithHeaderAnd { trace, header, block, .. } =
            self.trace_loader.get_tx_trace_with_header(tx_hash).await?;
        let (tx, rx) = unbounded_channel();

        let classifier = Classifier::new(self.libmdbx, tx, self.get_provider());

        let mut pricer = self.init_dex_pricer(block, None, quote_asset, rx).await?;
        let (decimals, tree) = classifier.build_block_tree(vec![trace], header).await;
        load_missing_decimals(
            self.get_provider(),
            self.libmdbx,
            block,
            decimals.tokens_decimal_fill,
        )
        .await;

        classifier.close();
        // triggers close
        drop(classifier);

        if let Some((p_block, pricing)) = pricer.next().await {
            assert!(p_block == block, "got pricing for the wrong block");
            Ok((tree, pricing))
        } else {
            Err(ClassifierTestUtilsError::DexPricingError)
        }
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
                    let (decimals, tree) = self
                        .classifier
                        .build_block_tree(block_info.traces, block_info.header)
                        .await;

                    load_missing_decimals(
                        self.get_provider(),
                        self.libmdbx,
                        block_info.block,
                        decimals.tokens_decimal_fill,
                    )
                    .await;

                    tree
                }),
        )
        .await)
    }

    pub async fn build_tree_txes_with_pricing(
        &self,
        tx_hashes: Vec<TxHash>,
        quote_asset: Address,
    ) -> Result<Vec<(BlockTree<Actions>, DexQuotes)>, ClassifierTestUtilsError> {
        let (tx, rx) = unbounded_channel();
        let classifier = Classifier::new(self.libmdbx, tx, self.get_provider());

        let mut start_block = 0;
        let mut end_block = 0;

        let mut trees = Vec::new();
        for block_info in self
            .trace_loader
            .get_tx_traces_with_header(tx_hashes)
            .await?
            .into_iter()
        {
            if start_block == 0 {
                start_block = block_info.block;
            }
            end_block = block_info.block;

            let (decimals, tree) = classifier
                .build_block_tree(block_info.traces, block_info.header)
                .await;
            load_missing_decimals(
                self.get_provider(),
                self.libmdbx,
                block_info.block,
                decimals.tokens_decimal_fill,
            )
            .await;
            trees.push(tree);
        }

        let mut pricer = self
            .init_dex_pricer(start_block, Some(end_block), quote_asset, rx)
            .await?;

        classifier.close();
        drop(classifier);

        let mut prices = Vec::new();

        while let Some((_, quotes)) = pricer.next().await {
            prices.push(quotes);
        }

        Ok(trees.into_iter().zip(prices.into_iter()).collect())
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

    pub async fn build_tree_block_with_pricing(
        &self,
        block: u64,
        quote_asset: Address,
    ) -> Result<(BlockTree<Actions>, DexQuotes), ClassifierTestUtilsError> {
        let BlockTracesWithHeaderAnd { traces, header, .. } = self
            .trace_loader
            .get_block_traces_with_header(block)
            .await?;

        let (tx, rx) = unbounded_channel();
        let classifier = Classifier::new(self.libmdbx, tx, self.get_provider());

        let mut pricer = self.init_dex_pricer(block, None, quote_asset, rx).await?;
        let (decimals, tree) = classifier.build_block_tree(traces, header).await;
        load_missing_decimals(
            self.get_provider(),
            self.libmdbx,
            block,
            decimals.tokens_decimal_fill,
        )
        .await;

        classifier.close();
        // triggers close
        drop(classifier);

        if let Some((p_block, pricing)) = pricer.next().await {
            assert!(p_block == block, "got pricing for the wrong block");
            Ok((tree, pricing))
        } else {
            Err(ClassifierTestUtilsError::DexPricingError)
        }
    }

    pub async fn contains_action(
        &self,
        tx_hash: TxHash,
        action_number_in_tx: usize,
        eq_action: Actions,
        tree_collect_fn: impl Fn(&Node<Actions>) -> (bool, bool),
    ) -> Result<(), ClassifierTestUtilsError> {
        let mut tree = self.build_tree_tx(tx_hash).await?;
        let root = tree.tx_roots.remove(0);
        let mut actions = root.collect(&tree_collect_fn);
        let action = actions.remove(action_number_in_tx);

        assert_eq!(eq_action, action, "got: {:#?} != given: {:#?}", action, eq_action);

        Ok(())
    }

    pub async fn is_missing_action(
        &self,
        _tx_hash: TxHash,
        _action_number_in_block: usize,
        _eq_action: Actions,
        _tree_collect_fn: impl Fn(&Node<Actions>) -> (bool, bool),
    ) -> Result<(), ClassifierTestUtilsError> {
        todo!()
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
    #[error("failed to read from libmdbx")]
    LibmdbxError,
    #[error("dex pricing failed")]
    DexPricingError,
}
