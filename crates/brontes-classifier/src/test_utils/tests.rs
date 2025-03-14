use std::{
    ops::Deref,
    sync::{
        atomic::{AtomicBool, Ordering::SeqCst},
        Arc,
    },
};

use alloy_primitives::{Address, TxHash};
use brontes_core::{
    decoding::TracingProvider, BlockTracesWithHeaderAnd, TraceLoader, TraceLoaderError,
    TxTracesWithHeaderAnd,
};
use brontes_database::{
    libmdbx::{LibmdbxReadWriter, LibmdbxReader},
    AddressToProtocolInfo, AddressToProtocolInfoData, TokenDecimals, TokenDecimalsData,
};
use brontes_pricing::{
    types::{DexPriceMsg, PoolUpdate},
    BrontesBatchPricer, GraphManager, Protocol,
};
use brontes_types::{
    db::{
        address_to_protocol_info::ProtocolInfo, dex::DexQuotes, token_info::TokenInfoWithAddress,
    },
    normalized_actions::{pool::NormalizedNewPool, NormalizedTransfer},
    structured_trace::TraceActions,
    tree::BlockTree,
    BrontesTaskManager, FastHashMap, TreeCollector, TreeSearchBuilder, UnboundedYapperReceiver,
};
use futures::{future::join_all, StreamExt};
use reth_db::DatabaseError;
use serde_json::Value;
use thiserror::Error;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::{Action, ActionCollection, Classifier, ProtocolClassifier};

pub struct ClassifierTestUtils {
    pub trace_loader: TraceLoader,
    classifier:       Classifier<'static, Box<dyn TracingProvider>, LibmdbxReadWriter>,

    dex_pricing_receiver: UnboundedReceiver<DexPriceMsg>,
}
impl ClassifierTestUtils {
    pub async fn new() -> Self {
        let trace_loader = TraceLoader::new().await;
        let (tx, rx) = unbounded_channel();
        let classifier = Classifier::new(trace_loader.libmdbx, tx, trace_loader.get_provider());
        Self { classifier, trace_loader, dex_pricing_receiver: rx }
    }

    pub fn get_tracing_provider(&self) -> Arc<Box<dyn TracingProvider>> {
        self.get_provider()
    }

    pub fn get_token_info(&self, address: Address) -> TokenInfoWithAddress {
        self.libmdbx.try_fetch_token_info(address).unwrap()
    }

    async fn init_dex_pricer(
        &self,
        block: u64,
        end_block: Option<u64>,
        quote_asset: Address,
        rx: UnboundedReceiver<DexPriceMsg>,
    ) -> Result<
        (Arc<AtomicBool>, BrontesBatchPricer<Box<dyn TracingProvider>>),
        ClassifierTestUtilsError,
    > {
        let pairs = self
            .libmdbx
            .protocols_created_before(block)
            .map_err(|_| ClassifierTestUtilsError::LibmdbxError)?;

        let pair_graph = GraphManager::init_from_db_state(pairs, None);

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
                .collect::<FastHashMap<_, _>>()
        } else {
            FastHashMap::default()
        };
        let ctr = Arc::new(AtomicBool::new(false));
        let ex = BrontesTaskManager::current().executor();

        Ok((
            ctr.clone(),
            BrontesBatchPricer::new(
                0,
                ctr.clone(),
                quote_asset,
                pair_graph,
                UnboundedYapperReceiver::new(rx, 10000, "test".into()),
                self.get_provider(),
                block,
                created_pools,
                ctr.clone(),
                None,
                ex,
            ),
        ))
    }

    pub fn get_pricing_receiver(&mut self) -> &mut UnboundedReceiver<DexPriceMsg> {
        &mut self.dex_pricing_receiver
    }

    pub async fn build_raw_tree_tx(
        &self,
        tx_hash: TxHash,
    ) -> Result<BlockTree<Action>, ClassifierTestUtilsError> {
        let TxTracesWithHeaderAnd { trace, header, .. } =
            self.trace_loader.get_tx_trace_with_header(tx_hash).await?;

        let tx_roots = self.classifier.build_tx_trees(vec![trace], &header).await;

        let mut tree = BlockTree::new(header, tx_roots.len());

        tx_roots.into_iter().for_each(|root_data| {
            tree.insert_root(root_data.root);
        });

        Ok(tree)
    }

    pub async fn build_raw_tree_txes(
        &self,
        tx_hashes: Vec<TxHash>,
    ) -> Result<Vec<BlockTree<Action>>, ClassifierTestUtilsError> {
        Ok(join_all(
            self.trace_loader
                .get_tx_traces_with_header(tx_hashes)
                .await?
                .into_iter()
                .map(|data| async move {
                    let tx_roots = self
                        .classifier
                        .build_tx_trees(data.traces, &data.header)
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
    ) -> Result<BlockTree<Action>, ClassifierTestUtilsError> {
        let TxTracesWithHeaderAnd { trace, header, .. } =
            self.trace_loader.get_tx_trace_with_header(tx_hash).await?;
        Ok(self
            .classifier
            .build_block_tree(vec![trace], header, true)
            .await)
    }

    pub async fn setup_pricing_for_bench(
        &self,
        block: u64,
        quote_asset: Address,
        needs_tokens: Vec<Address>,
    ) -> Result<
        (BrontesBatchPricer<Box<dyn TracingProvider>>, UnboundedSender<DexPriceMsg>),
        ClassifierTestUtilsError,
    > {
        let BlockTracesWithHeaderAnd { traces, header, block, .. } = self
            .trace_loader
            .get_block_traces_with_header(block)
            .await?;
        let (tx, rx) = unbounded_channel();

        let classifier = Classifier::new(self.libmdbx, tx.clone(), self.get_provider());
        let _tree = classifier.build_block_tree(traces, header, true).await;

        needs_tokens.iter().for_each(|token| {
            let update = DexPriceMsg::Update(PoolUpdate {
                block,
                tx_idx: 0,
                logs: vec![],
                action: make_fake_transfer(*token),
            });
            tx.send(update).unwrap();
        });
        let (ctr, pricer) = self.init_dex_pricer(block, None, quote_asset, rx).await?;
        classifier.close();
        ctr.store(true, SeqCst);

        Ok((pricer, tx))
    }

    pub async fn send_traces_for_block(
        &self,
        block: u64,
        tx: UnboundedSender<DexPriceMsg>,
    ) -> Result<(), ClassifierTestUtilsError> {
        let BlockTracesWithHeaderAnd { traces, header, .. } = self
            .trace_loader
            .get_block_traces_with_header(block)
            .await?;

        let classifier = Classifier::new(self.libmdbx, tx, self.get_provider());
        let _tree = classifier.build_block_tree(traces, header, true).await;

        Ok(())
    }

    pub async fn setup_pricing_for_bench_post_init(
        &self,
        block: u64,
        past_n: u64,
        quote_asset: Address,
        needs_tokens: Vec<Address>,
    ) -> Result<
        (
            BrontesBatchPricer<Box<dyn TracingProvider>>,
            UnboundedSender<DexPriceMsg>,
            Arc<AtomicBool>,
        ),
        ClassifierTestUtilsError,
    > {
        let mut range_traces = self
            .trace_loader
            .get_block_traces_with_header_range(block, block + past_n)
            .await?;

        let (tx, rx) = unbounded_channel();

        let BlockTracesWithHeaderAnd { traces, header, .. } = range_traces.remove(0);

        let classifier = Classifier::new(self.libmdbx, tx.clone(), self.get_provider());
        let _tree = classifier.build_block_tree(traces, header, true).await;

        needs_tokens.iter().for_each(|token| {
            let update = DexPriceMsg::Update(PoolUpdate {
                block,
                tx_idx: 0,
                logs: vec![],
                action: make_fake_transfer(*token),
            });
            tx.send(update).unwrap();
        });

        let (ctr, mut pricer) = self.init_dex_pricer(block, None, quote_asset, rx).await?;

        // send rest of updates
        for BlockTracesWithHeaderAnd { traces, header, .. } in range_traces {
            classifier.build_block_tree(traces, header, true).await;
        }

        ctr.store(true, SeqCst);
        while (pricer.next().await).is_some() {}
        ctr.store(false, SeqCst);

        Ok((pricer, tx, ctr))
    }

    pub async fn build_tree_tx_with_pricing(
        &self,
        tx_hash: TxHash,
        quote_asset: Address,
        needs_tokens: Vec<Address>,
    ) -> Result<(BlockTree<Action>, Option<DexQuotes>), ClassifierTestUtilsError> {
        let TxTracesWithHeaderAnd { trace, header, block, .. } =
            self.trace_loader.get_tx_trace_with_header(tx_hash).await?;
        let (tx, rx) = unbounded_channel();

        let classifier = Classifier::new(self.libmdbx, tx.clone(), self.get_provider());
        let tree = classifier.build_block_tree(vec![trace], header, true).await;

        needs_tokens.iter().for_each(|token| {
            let update = DexPriceMsg::Update(PoolUpdate {
                block,
                tx_idx: 0,
                logs: vec![],
                action: make_fake_transfer(*token),
            });

            tx.send(update).unwrap();
        });
        let (ctr, mut pricer) = self.init_dex_pricer(block, None, quote_asset, rx).await?;
        classifier.close();
        ctr.store(true, SeqCst);
        // triggers close

        let price = if let Some((_p_block, pricing)) = pricer.next().await {
            Some(pricing)
        } else {
            return Err(ClassifierTestUtilsError::DexPricingError);
        };

        Ok((tree, price))
    }

    pub async fn build_tree_txes(
        &self,
        tx_hashes: Vec<TxHash>,
    ) -> Result<Vec<BlockTree<Action>>, ClassifierTestUtilsError> {
        Ok(join_all(
            self.trace_loader
                .get_tx_traces_with_header(tx_hashes)
                .await?
                .into_iter()
                .map(|block_info| async move {
                    self.classifier
                        .build_block_tree(block_info.traces, block_info.header, true)
                        .await
                }),
        )
        .await)
    }

    pub async fn test_pool_token_order(
        &self,
        token_0: Address,
        token_1: Address,
        pool: Address,
    ) -> bool {
        let pool = self.libmdbx.get_protocol_details_sorted(pool).unwrap();

        pool.token0 == token_0 && pool.token1 == token_1
    }

    pub async fn build_tree_txes_with_pricing(
        &self,
        tx_hashes: Vec<TxHash>,
        quote_asset: Address,
        needs_tokens: Vec<Address>,
    ) -> Result<Vec<(BlockTree<Action>, DexQuotes)>, ClassifierTestUtilsError> {
        let (tx, rx) = unbounded_channel();
        let classifier = Classifier::new(self.libmdbx, tx.clone(), self.get_provider());

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
                .build_block_tree(block_info.traces, block_info.header, true)
                .await;

            trees.push(tree);
        }

        (start_block..=end_block).for_each(|block| {
            needs_tokens.iter().for_each(|token| {
                let update = DexPriceMsg::Update(PoolUpdate {
                    block,
                    tx_idx: 0,
                    logs: vec![],
                    action: make_fake_transfer(*token),
                });
                tx.send(update).unwrap();
            });
        });

        let (ctr, mut pricer) = self
            .init_dex_pricer(start_block, None, quote_asset, rx)
            .await?;
        classifier.close();
        ctr.store(true, SeqCst);

        let mut prices = Vec::new();

        while let Some((_p_block, quotes)) = pricer.next().await {
            prices.push(quotes);
        }

        Ok(trees.into_iter().zip(prices.into_iter()).collect())
    }

    pub async fn build_block_tree(
        &self,
        block: u64,
    ) -> Result<BlockTree<Action>, ClassifierTestUtilsError> {
        let BlockTracesWithHeaderAnd { traces, header, .. } = self
            .trace_loader
            .get_block_traces_with_header(block)
            .await?;
        let tree = self.classifier.build_block_tree(traces, header, true).await;

        Ok(tree)
    }

    pub async fn build_block_tree_with_pricing(
        &self,
        block: u64,
        quote_asset: Address,
        needs_tokens: Vec<Address>,
    ) -> Result<(BlockTree<Action>, Option<DexQuotes>), ClassifierTestUtilsError> {
        let BlockTracesWithHeaderAnd { traces, header, .. } = self
            .trace_loader
            .get_block_traces_with_header(block)
            .await?;

        let (tx, rx) = unbounded_channel();
        let classifier = Classifier::new(self.libmdbx, tx.clone(), self.get_provider());
        let tree = classifier.build_block_tree(traces, header, true).await;

        needs_tokens.iter().for_each(|token| {
            let update = DexPriceMsg::Update(PoolUpdate {
                block,
                tx_idx: 0,
                logs: vec![],
                action: make_fake_transfer(*token),
            });
            tx.send(update).unwrap();
        });

        let (ctr, mut pricer) = self.init_dex_pricer(block, None, quote_asset, rx).await?;
        classifier.close();
        ctr.store(true, SeqCst);

        let price = if let Some((_p_block, pricing)) = pricer.next().await {
            Some(pricing)
        } else {
            return Err(ClassifierTestUtilsError::DexPricingError);
        };

        Ok((tree, price))
    }

    pub async fn contains_action_except(
        &self,
        tx_hash: TxHash,
        action_number_in_tx: usize,
        eq_action: Action,
        tree_collect_builder: TreeSearchBuilder<Action>,
        ignore_fields: &[&str],
    ) -> Result<(), ClassifierTestUtilsError> {
        let mut tree = self.build_tree_tx(tx_hash).await?;

        assert!(!tree.tx_roots.is_empty(), "empty tree. most likely an invalid hash");

        let root = tree.tx_roots.remove(0);
        let mut actions = root.collect(&tree_collect_builder);
        assert!(
            !actions.is_empty(),
            "no actions collected. protocol is either missing from db or not added to dispatch"
        );
        assert!(actions.len() > action_number_in_tx, "incorrect action index");

        let action = actions.remove(action_number_in_tx);

        assert!(
            partially_eq(&action, &eq_action, ignore_fields),
            "got: {:#?} != given: {:#?}",
            action,
            eq_action
        );

        Ok(())
    }

    pub async fn detects_protocol_at(
        &self,
        tx_hash: TxHash,
        index: usize,
        protocol: Protocol,
        tree_collect_builder: TreeSearchBuilder<Action>,
    ) -> Result<(), ClassifierTestUtilsError> {
        let mut tree = self.build_tree_tx(tx_hash).await?;

        assert!(!tree.tx_roots.is_empty(), "empty tree. most likely a invalid hash");

        let root = tree.tx_roots.remove(0);
        let mut actions = root.collect(&tree_collect_builder);
        assert!(
            !actions.is_empty(),
            "no actions collected. protocol is either missing
                from db or not added to dispatch"
        );

        let action = actions.remove(index);
        assert_eq!(
            protocol,
            action.get_protocol(),
            "got: {:#?} != given: {:#?}",
            action.get_protocol(),
            protocol
        );

        Ok(())
    }

    pub async fn contains_action(
        &self,
        tx_hash: TxHash,
        action_number_in_tx: usize,
        eq_action: Action,
        tree_collect_builder: TreeSearchBuilder<Action>,
    ) -> Result<(), ClassifierTestUtilsError> {
        let mut tree = self.build_tree_tx(tx_hash).await?;

        assert!(!tree.tx_roots.is_empty(), "empty tree. most likely a invalid hash");

        let root = tree.tx_roots.remove(0);
        let mut actions = root.collect(&tree_collect_builder);
        assert!(
            !actions.is_empty(),
            "no actions collected. protocol is either missing
                from db or not added to dispatch"
        );
        assert!(actions.len() > action_number_in_tx, "incorrect action index");

        let action = actions.remove(action_number_in_tx);

        assert_eq!(eq_action, action, "got: {:#?} != given: {:#?}", action, eq_action);

        Ok(())
    }

    pub async fn has_no_actions(
        &self,
        tx_hash: TxHash,
        tree_collect_builder: TreeSearchBuilder<Action>,
    ) -> Result<(), ClassifierTestUtilsError> {
        let mut tree = self.build_tree_tx(tx_hash).await?;
        let root = tree.tx_roots.remove(0);
        let actions = root.collect(&tree_collect_builder);

        assert!(actions.is_empty(), "found: {:#?}", actions);
        Ok(())
    }

    pub async fn test_protocol_classification(
        &self,
        tx_hash: TxHash,
        protocol: ProtocolInfo,
        address: Address,
        cmp_fn: impl Fn(Option<Action>),
    ) -> Result<(), ClassifierTestUtilsError> {
        // write protocol to libmdbx
        self.libmdbx
            .db
            .write_table::<AddressToProtocolInfo, AddressToProtocolInfoData>(&[
                AddressToProtocolInfoData { key: address, value: protocol },
            ])?;

        let TxTracesWithHeaderAnd { trace, block, .. } =
            self.get_tx_trace_with_header(tx_hash).await?;

        let trace = trace
            .trace
            .into_iter()
            .find(|t| t.get_to_address() == address)
            .ok_or_else(|| ClassifierTestUtilsError::ProtocolClassifierError(address))?;

        let dispatcher = ProtocolClassifier::default();

        let call_info = trace.get_callframe_info();

        let result = dispatcher.dispatch(call_info, self.trace_loader.libmdbx, block, 0);

        cmp_fn(result.map(|i| i.1));

        Ok(())
    }

    pub async fn test_discovery_classification(
        &self,
        txes: TxHash,
        _created_pool: Address,
        cmp_fn: impl Fn(Vec<NormalizedNewPool>),
    ) -> Result<(), ClassifierTestUtilsError> {
        let TxTracesWithHeaderAnd { trace, header, .. } =
            self.get_tx_trace_with_header(txes).await?;

        let (tx, _rx) = unbounded_channel();
        let classifier = Classifier::new(self.libmdbx, tx.clone(), self.get_provider());
        let tree = classifier.build_block_tree(vec![trace], header, true).await;
        let res = Arc::new(tree)
            .collect(&txes, TreeSearchBuilder::default().with_action(Action::is_new_pool))
            .split_actions(Action::try_new_pool);

        cmp_fn(res);

        Ok(())
    }

    pub fn ensure_protocol(
        &self,
        protocol: Protocol,
        address: Address,
        token0: Address,
        token1: Option<Address>,
        token2: Option<Address>,
        token3: Option<Address>,
        token4: Option<Address>,
        curve_lp_token: Option<Address>,
    ) {
        if let Err(e) = self
            .libmdbx
            .db
            .write_table::<AddressToProtocolInfo, AddressToProtocolInfoData>(&[
                AddressToProtocolInfoData {
                    key:   address,
                    value: ProtocolInfo {
                        protocol,
                        token0,
                        token1: token1.unwrap_or_default(),
                        token2,
                        token3,
                        token4,
                        curve_lp_token,
                        init_block: 0,
                    },
                },
            ])
        {
            tracing::error!(error=%e, %protocol, ?address, "failed to ensure protocol is in db");
        }
    }

    pub fn ensure_token(&self, token: TokenInfoWithAddress) {
        if let Err(e) = self
            .libmdbx
            .db
            .write_table::<TokenDecimals, TokenDecimalsData>(&[TokenDecimalsData {
                key:   token.address,
                value: brontes_types::db::token_info::TokenInfo {
                    decimals: token.decimals,
                    symbol:   token.symbol.clone(),
                },
            }])
        {
            tracing::error!(error=%e, ?token, "failed to ensure token is in db");
        }
    }
}

impl Deref for ClassifierTestUtils {
    type Target = TraceLoader;

    fn deref(&self) -> &Self::Target {
        &self.trace_loader
    }
}

fn partially_eq<T: serde::Serialize>(a: &T, b: &T, ignore_fields: &[&str]) -> bool {
    let a_json = serde_json::to_value(a).unwrap();
    let b_json = serde_json::to_value(b).unwrap();

    fn filter_fields(value: &Value, ignore_fields: &[&str]) -> Value {
        match value {
            Value::Object(map) => {
                let filtered_map: serde_json::Map<String, Value> = map
                    .iter()
                    .filter(|(k, _)| !ignore_fields.contains(&k.as_str()))
                    .map(|(k, v)| (k.clone(), filter_fields(v, ignore_fields)))
                    .collect();
                Value::Object(filtered_map)
            }
            _ => value.clone(),
        }
    }

    let a_filtered = filter_fields(&a_json, ignore_fields);
    let b_filtered = filter_fields(&b_json, ignore_fields);

    a_filtered == b_filtered
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
    ProtocolClassifierError(Address),
}

/// Makes a swap for initializing a virtual pool with the quote token.
/// this swap is empty such that we don't effect the state
fn make_fake_transfer(addr: Address) -> Action {
    let t_in = TokenInfoWithAddress {
        inner:   brontes_types::db::token_info::TokenInfo { decimals: 0, symbol: String::new() },
        address: addr,
    };

    Action::Transfer(NormalizedTransfer { token: t_in, ..Default::default() })
}
