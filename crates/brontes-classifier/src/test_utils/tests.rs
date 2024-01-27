use std::{collections::HashMap, ops::Deref};

use alloy_primitives::{Address, TxHash};
use brontes_core::{
    decoding::TracingProvider, BlockTracesWithHeaderAnd, TraceLoader, TraceLoaderError,
    TxTracesWithHeaderAnd,
};
use brontes_database::{
    libmdbx::{
        types::address_to_protocol::AddressToProtocolData, LibmdbxReadWriter, LibmdbxReader,
    },
    AddressToProtocol,
};
use brontes_pricing::{
    types::{DexPriceMsg, DiscoveredPool, PoolUpdate},
    BrontesBatchPricer, GraphManager, Protocol,
};
use brontes_types::{
    db::dex::DexQuotes,
    structured_trace::TraceActions,
    tree::{BlockTree, Node},
};
use futures::{future::join_all, StreamExt};
use reth_db::DatabaseError;
use reth_rpc_types::trace::parity::Action;
use thiserror::Error;
use tokio::{
    runtime::Handle,
    sync::mpsc::{unbounded_channel, UnboundedReceiver},
};

use crate::{
    ActionCollection, Actions, Classifier, DiscoveryProtocols, FactoryDecoderDispatch,
    ProtocolClassifications,
};

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

    pub fn new_with_rt(handle: Handle) -> Self {
        let trace_loader = TraceLoader::new_with_rt(handle);
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
        let tree = self.classifier.build_block_tree(vec![trace], header).await;

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
        let tree = classifier.build_block_tree(vec![trace], header).await;
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
                    let tree = self
                        .classifier
                        .build_block_tree(block_info.traces, block_info.header)
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

            let tree = classifier
                .build_block_tree(block_info.traces, block_info.header)
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
        let tree = self.classifier.build_block_tree(traces, header).await;

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
        let tree = classifier.build_block_tree(traces, header).await;

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

    pub async fn has_no_actions(
        &self,
        tx_hash: TxHash,
        tree_collect_fn: impl Fn(&Node<Actions>) -> (bool, bool),
    ) -> Result<(), ClassifierTestUtilsError> {
        let mut tree = self.build_tree_tx(tx_hash).await?;
        let root = tree.tx_roots.remove(0);
        let actions = root.collect(&tree_collect_fn);

        assert!(actions.is_empty(), "found: {:#?}", actions);
        Ok(())
    }

    pub async fn test_protocol_classification(
        &self,
        tx_hash: TxHash,
        protocol: Protocol,
        address: Address,
        cmp_fn: impl Fn(Option<(PoolUpdate, Actions)>),
    ) -> Result<(), ClassifierTestUtilsError> {
        // write protocol to libmdbx
        self.libmdbx
            .0
            .write_table::<AddressToProtocol, AddressToProtocolData>(&vec![
                AddressToProtocolData { address, classifier_name: protocol },
            ])?;

        let TxTracesWithHeaderAnd { trace, block, .. } =
            self.get_tx_trace_with_header(tx_hash).await?;

        let trace = trace
            .trace
            .into_iter()
            .find(|t| t.get_to_address() == address)
            .ok_or_else(|| ClassifierTestUtilsError::ProtocolClassificationError(address))?;

        let dispatcher = ProtocolClassifications::default();

        let from_address = trace.get_from_addr();
        let target_address = trace.get_to_address();

        let call_data = trace.get_calldata();
        let return_bytes = trace.get_return_calldata();

        let result = dispatcher.dispatch(
            0,
            call_data.clone(),
            return_bytes.clone(),
            from_address,
            target_address,
            trace.msg_sender,
            &trace.logs,
            self.trace_loader.libmdbx,
            block,
            0,
        );

        cmp_fn(result);

        Ok(())
    }

    pub async fn test_discovery_classification(
        &self,
        tx: TxHash,
        created_pool: Address,
        cmp_fn: impl Fn(Vec<DiscoveredPool>),
    ) -> Result<(), ClassifierTestUtilsError> {
        let TxTracesWithHeaderAnd { trace, .. } = self.get_tx_trace_with_header(tx).await?;

        let found_trace = trace
            .trace
            .iter()
            .filter(|t| t.is_create())
            .find(|t| t.get_create_output() == created_pool)
            .ok_or_else(|| ClassifierTestUtilsError::DiscoveryError(created_pool))?;

        let mut trace_addr = found_trace.get_trace_address();

        if trace_addr.len() > 1 {
            trace_addr.pop().unwrap();
        } else {
            return Err(ClassifierTestUtilsError::ProtocolDiscoveryError(created_pool))
        };

        let p_trace = trace
            .trace
            .iter()
            .find(|f| f.get_trace_address() == trace_addr)
            .ok_or_else(|| ClassifierTestUtilsError::ProtocolDiscoveryError(created_pool))?;

        let Action::Call(call) = &p_trace.trace.action else { panic!() };

        let from_address = found_trace.get_from_addr();
        let created_addr = found_trace.get_create_output();
        let dispatcher = DiscoveryProtocols::default();
        let call_data = call.input.clone();
        let tracer = self.trace_loader.get_provider();

        let res = dispatcher
            .dispatch(tracer.clone(), from_address, created_addr, call_data.clone())
            .await;

        cmp_fn(res);

        Ok(())
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
    #[error(transparent)]
    DatabaseError(#[from] DatabaseError),
    #[error("failed to read from libmdbx")]
    LibmdbxError,
    #[error("dex pricing failed")]
    DexPricingError,
    #[error("couldn't find trace for address: {0:?}")]
    DiscoveryError(Address),
    #[error("couldn't find parent node for created pool {0:?}")]
    ProtocolDiscoveryError(Address),
    #[error("couldn't find trace that matched {0:?}")]
    ProtocolClassificationError(Address),
}
