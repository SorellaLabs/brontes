use std::{
    collections::HashMap,
    ops::Deref,
    sync::{
        atomic::{AtomicBool, Ordering::SeqCst},
        Arc,
    },
};

use alloy_primitives::{Address, TxHash, U256};
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
    normalized_actions::{pool::NormalizedNewPool, NormalizedSwap},
    pair::Pair,
    structured_trace::TraceActions,
    tree::BlockTree,
    TreeSearchBuilder,
};
use futures::{future::join_all, StreamExt};
use itertools::Itertools;
use malachite::{num::basic::traits::Zero, Rational};
use reth_db::DatabaseError;
use reth_rpc_types::trace::parity::Action;
use thiserror::Error;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::{
    ActionCollection, Actions, Classifier, DiscoveryProtocols, FactoryDiscoveryDispatch,
    ProtocolClassifications,
};

pub struct ClassifierTestUtils {
    trace_loader: TraceLoader,
    classifier:   Classifier<'static, Box<dyn TracingProvider>, LibmdbxReadWriter>,

    dex_pricing_receiver: UnboundedReceiver<DexPriceMsg>,
}
impl ClassifierTestUtils {
    pub async fn new() -> Self {
        let trace_loader = TraceLoader::new().await;
        let (tx, rx) = unbounded_channel();
        let classifier = Classifier::new(trace_loader.libmdbx, tx, trace_loader.get_provider());
        Self { classifier, trace_loader, dex_pricing_receiver: rx }
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
        (Arc<AtomicBool>, BrontesBatchPricer<Box<dyn TracingProvider>, LibmdbxReadWriter>),
        ClassifierTestUtilsError,
    > {
        let pairs = self
            .libmdbx
            .protocols_created_before(block)
            .map_err(|_| ClassifierTestUtilsError::LibmdbxError)?;

        let pair_graph = GraphManager::init_from_db_state(pairs, HashMap::default(), self.libmdbx);

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
        let ctr = Arc::new(AtomicBool::new(false));

        Ok((
            ctr.clone(),
            BrontesBatchPricer::new(
                ctr,
                quote_asset,
                pair_graph,
                rx,
                self.get_provider(),
                block,
                created_pools,
            ),
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

    /// returns true if we need to query dex tokens
    fn need_dex_quotes(
        &self,
        block: u64,
        quote_token: Address,
        quotes: Option<&DexQuotes>,
        needs_tokens: &[Address],
        tx: UnboundedSender<DexPriceMsg>,
    ) -> bool {
        if let Some(quote) = quotes {
            needs_tokens
                .iter()
                .zip(vec![quote_token].into_iter().cycle())
                .map(|(token, quote)| Pair(*token, quote))
                .filter(|pair| !quote.has_quote(pair, 0))
                .map(|pair| {
                    let update = DexPriceMsg::Update(PoolUpdate {
                        block,
                        tx_idx: 0,
                        logs: vec![],
                        action: make_fake_swap(pair),
                    });
                    tx.send(update).unwrap();
                })
                .count()
                != 0
        } else {
            needs_tokens
                .iter()
                .zip(vec![quote_token].into_iter().cycle())
                .map(|(token, quote)| Pair(*token, quote))
                .for_each(|pair| {
                    let update = DexPriceMsg::Update(PoolUpdate {
                        block,
                        tx_idx: 0,
                        logs: vec![],
                        action: make_fake_swap(pair),
                    });
                    tx.send(update).unwrap();
                });
            true
        }
    }

    pub async fn build_tree_tx(
        &self,
        tx_hash: TxHash,
    ) -> Result<BlockTree<Actions>, ClassifierTestUtilsError> {
        let TxTracesWithHeaderAnd { trace, header, .. } =
            self.trace_loader.get_tx_trace_with_header(tx_hash).await?;
        Ok(self.classifier.build_block_tree(vec![trace], header).await)
    }

    pub async fn build_tree_tx_with_pricing(
        &self,
        tx_hash: TxHash,
        quote_asset: Address,
        needs_tokens: Vec<Address>,
    ) -> Result<(BlockTree<Actions>, Option<DexQuotes>), ClassifierTestUtilsError> {
        let TxTracesWithHeaderAnd { trace, header, block, .. } =
            self.trace_loader.get_tx_trace_with_header(tx_hash).await?;
        let (tx, rx) = unbounded_channel();

        let classifier = Classifier::new(self.libmdbx, tx.clone(), self.get_provider());
        let tree = classifier.build_block_tree(vec![trace], header).await;

        let price = if let Ok(m) = self.libmdbx.get_dex_quotes(block) { Some(m) } else { None };

        let price = if self.need_dex_quotes(block, quote_asset, price.as_ref(), &needs_tokens, tx)
            || price.as_ref().map(|v| v.0.is_empty()) == Some(true)
        {
            let (ctr, mut pricer) = self.init_dex_pricer(block, None, quote_asset, rx).await?;
            classifier.close();

            ctr.store(true, SeqCst);
            // triggers close
            drop(classifier);

            if let Some((_p_block, pricing)) = pricer.next().await {
                Some(pricing)
            } else {
                return Err(ClassifierTestUtilsError::DexPricingError)
            }
        } else {
            price
        };

        Ok((tree, price))
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
                    self.classifier
                        .build_block_tree(block_info.traces, block_info.header)
                        .await
                }),
        )
        .await)
    }

    pub async fn build_tree_txes_with_pricing(
        &self,
        tx_hashes: Vec<TxHash>,
        quote_asset: Address,
        needs_tokens: Vec<Address>,
    ) -> Result<Vec<(BlockTree<Actions>, DexQuotes)>, ClassifierTestUtilsError> {
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
                .build_block_tree(block_info.traces, block_info.header)
                .await;

            trees.push(tree);
        }

        let mut possible_price = Vec::new();
        let mut failed = false;

        for block_num in start_block..=end_block {
            match self.libmdbx.fetch_dex_quotes(block_num) {
                Ok(dex_quotes) => {
                    possible_price.push((block_num, dex_quotes));
                }
                Err(_) => {
                    failed = true;
                    break
                }
            }
        }

        let prices = if possible_price
            .iter()
            .map(|(block, price)| {
                if price.0.is_empty() {
                    return true
                };

                self.need_dex_quotes(*block, quote_asset, Some(price), &needs_tokens, tx.clone())
            })
            .any(|f| f)
            || failed
        {
            let (ctr, mut pricer) = self
                .init_dex_pricer(start_block, None, quote_asset, rx)
                .await?;
            classifier.close();

            ctr.store(true, SeqCst);
            // triggers close
            drop(classifier);

            let mut prices = Vec::new();

            while let Some((_p_block, quotes)) = pricer.next().await {
                prices.push(quotes);
            }
            prices
        } else {
            possible_price.into_iter().map(|(_, q)| q).collect_vec()
        };

        Ok(trees.into_iter().zip(prices.into_iter()).collect())
    }

    pub async fn build_block_tree(
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

    pub async fn build_block_tree_with_pricing(
        &self,
        block: u64,
        quote_asset: Address,
        needs_tokens: Vec<Address>,
    ) -> Result<(BlockTree<Actions>, Option<DexQuotes>), ClassifierTestUtilsError> {
        let BlockTracesWithHeaderAnd { traces, header, .. } = self
            .trace_loader
            .get_block_traces_with_header(block)
            .await?;

        let (tx, rx) = unbounded_channel();
        let classifier = Classifier::new(self.libmdbx, tx.clone(), self.get_provider());
        let tree = classifier.build_block_tree(traces, header).await;

        let price = if let Ok(m) = self.libmdbx.get_dex_quotes(block) { Some(m) } else { None };

        let price = if self.need_dex_quotes(block, quote_asset, price.as_ref(), &needs_tokens, tx)
            || price.as_ref().map(|v| v.0.is_empty()) == Some(true)
        {
            let (ctr, mut pricer) = self.init_dex_pricer(block, None, quote_asset, rx).await?;
            classifier.close();

            ctr.store(true, SeqCst);
            // triggers close
            drop(classifier);

            if let Some((_p_block, pricing)) = pricer.next().await {
                Some(pricing)
            } else {
                return Err(ClassifierTestUtilsError::DexPricingError)
            }
        } else {
            price
        };

        Ok((tree, price))
    }

    pub async fn contains_action(
        &self,
        tx_hash: TxHash,
        action_number_in_tx: usize,
        eq_action: Actions,
        tree_collect_builder: TreeSearchBuilder<Actions>,
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
        tree_collect_builder: TreeSearchBuilder<Actions>,
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
        cmp_fn: impl Fn(Option<Actions>),
    ) -> Result<(), ClassifierTestUtilsError> {
        // write protocol to libmdbx
        self.libmdbx
            .0
            .write_table::<AddressToProtocolInfo, AddressToProtocolInfoData>(&vec![
                AddressToProtocolInfoData { key: address, value: protocol },
            ])?;

        let TxTracesWithHeaderAnd { trace, block, .. } =
            self.get_tx_trace_with_header(tx_hash).await?;

        let trace = trace
            .trace
            .into_iter()
            .find(|t| t.get_to_address() == address)
            .ok_or_else(|| ClassifierTestUtilsError::ProtocolClassificationError(address))?;

        let dispatcher = ProtocolClassifications::default();

        let call_info = trace.get_callframe_info();

        let result = dispatcher.dispatch(call_info, self.trace_loader.libmdbx, block, 0);

        cmp_fn(result.map(|i| i.1));

        Ok(())
    }

    pub async fn test_discovery_classification(
        &self,
        tx: TxHash,
        created_pool: Address,
        cmp_fn: impl Fn(Vec<NormalizedNewPool>),
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
        let calldata = call.input.clone();
        let trace_index = found_trace.trace_idx;

        let res = DiscoveryProtocols::default()
            .dispatch(self.get_provider(), from_address, created_addr, trace_index, calldata)
            .await;

        cmp_fn(res);

        Ok(())
    }

    pub fn ensure_protocol(
        &self,
        protocol: Protocol,
        address: Address,
        token0: Address,
        token1: Address,
        token2: Option<Address>,
        token3: Option<Address>,
        token4: Option<Address>,
        curve_lp_token: Option<Address>,
    ) {
        if let Err(e) = self
            .libmdbx
            .0
            .write_table::<AddressToProtocolInfo, AddressToProtocolInfoData>(&vec![
                AddressToProtocolInfoData {
                    key:   address,
                    value: ProtocolInfo {
                        protocol,
                        token0,
                        token1,
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
            .0
            .write_table::<TokenDecimals, TokenDecimalsData>(&vec![TokenDecimalsData {
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

/// Makes a swap for initializing a virtual pool with the quote token.
/// this swap is empty such that we don't effect the state
const fn make_fake_swap(pair: Pair) -> Actions {
    let t_in = TokenInfoWithAddress {
        inner:   brontes_types::db::token_info::TokenInfo { decimals: 0, symbol: String::new() },
        address: pair.0,
    };

    let t_out = TokenInfoWithAddress {
        inner:   brontes_types::db::token_info::TokenInfo { decimals: 0, symbol: String::new() },
        address: pair.1,
    };

    Actions::Swap(NormalizedSwap {
        protocol:    Protocol::Unknown,
        trace_index: 0,
        from:        Address::ZERO,
        recipient:   Address::ZERO,
        pool:        Address::ZERO,
        token_in:    t_in,
        token_out:   t_out,
        amount_in:   Rational::ZERO,
        amount_out:  Rational::ZERO,
        msg_value:   U256::ZERO,
    })
}
