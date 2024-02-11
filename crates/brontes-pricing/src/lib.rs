//! [`BrontesBatchPricer`] calculates and track the prices of tokens
//! on decentralized exchanges on a per-transaction basis. It builds and
//! maintains a main token graph which is used to derive smaller subgraphs used
//! to price tokens relative to a defined quote token.
//!
//! ## Core Functionality
//!
//! ### Subgraph Utilization
//! The system leverages subgraphs, which are smaller, focused graph structures
//! extracted from the larger token graph. Subgraphs are built when a classified
//! event occurs on a token. When this occurs a subgraph is made for the pair if
//! one doesn't already exist. This allows for fast computation of a tokens
//! price. These subgraphs constantly update with new blocks, updating their
//! nodes and edges to reflect new liquidity pools.  
//!
//! ### Graph Management
//! The system adds new pools to the token graph as they appear in new blocks,
//! ensuring that all valid trading paths are represented.
//!
//! ### Lazy Loading
//! New pools and their states are fetched as required, optimizing resource
//! usage and performance.
use alloy_primitives::U256;
use brontes_types::normalized_actions::pool::NormalizedPoolConfigUpdate;
mod graphs;
pub mod protocols;
pub mod types;
use std::sync::atomic::Ordering::SeqCst;

#[cfg(test)]
pub mod test_utils;

use std::{
    collections::{hash_map::Entry, HashMap, HashSet, VecDeque},
    sync::{atomic::AtomicBool, Arc},
    task::{Context, Poll},
};

use alloy_primitives::Address;
pub use brontes_types::price_graph_types::{
    PoolPairInfoDirection, PoolPairInformation, SubGraphEdge, SubGraphsEntry,
};
use brontes_types::{
    db::{
        dex::{DexPrices, DexQuotes},
        token_info::TokenInfoWithAddress,
        traits::{LibmdbxReader, LibmdbxWriter},
    },
    normalized_actions::{Actions, NormalizedSwap},
    pair::Pair,
    traits::TracingProvider,
};
use futures::{Stream, StreamExt};
pub use graphs::{AllPairGraph, GraphManager, VerificationResults};
use itertools::Itertools;
use malachite::{
    num::basic::traits::{One, Zero},
    Rational,
};
use protocols::lazy::{LazyExchangeLoader, LazyResult, LoadResult};
pub use protocols::{Protocol, *};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::error;
use types::{DexPriceMsg, PoolUpdate};

use crate::types::PoolState;

/// # Brontes Batch Pricer
///
/// [`BrontesBatchPricer`] establishes a token graph for pricing tokens against
/// a chosen quote token, highlighting differences between centralized and
/// decentralized exchange prices.
///
/// ## Workflow
/// The system operates on a block-by-block basis as follows:
///
/// 1) Incorporates new pools into the token graph with each new highest block
/// from the update channel.
///
/// 2) Uses a lazy loader to fetch data for all newly involved pools.
///
/// 3) Collects and buffers state transitions of all pools.
///
/// 4) After completing lazy loading, applies state transitions sequentially,
/// updating the price in the state map.
///
/// 5) Processes and returns formatted data from the applied state transitions
/// before proceeding to the next block.
pub struct BrontesBatchPricer<T: TracingProvider, DB: LibmdbxWriter + LibmdbxReader> {
    quote_asset:     Address,
    current_block:   u64,
    completed_block: u64,
    finished:        Arc<AtomicBool>,

    /// receiver from classifier, classifier is ran sequentially to guarantee
    /// order
    update_rx:       UnboundedReceiver<DexPriceMsg>,
    /// holds the state transfers and state void overrides for the given block.
    /// it works by processing all state transitions for a block and
    /// allowing lazy loading to occur. Once lazy loading has occurred and there
    /// are no more events for the current block, all the state transitions
    /// are applied in order with the price at the transaction index being
    /// calculated and inserted into the results and returned.
    buffer:          StateBuffer,
    /// holds new graph nodes / edges that can be added at every given block.
    /// this is done to ensure any route from a base to our quote asset will
    /// only pass though valid created pools.
    new_graph_pairs: HashMap<Address, (Protocol, Pair)>,
    /// manages all graph related items
    graph_manager:   GraphManager<DB>,
    /// lazy loads dex pairs so we only fetch init state that is needed
    lazy_loader:     LazyExchangeLoader<T>,
    dex_quotes:      HashMap<u64, DexQuotes>,
    /// when we are pulling from the channel, because its not peekable we always
    /// pull out one more than we want. this acts as a cache for it
    overlap_update:  Option<PoolUpdate>,
}

impl<T: TracingProvider, DB: LibmdbxWriter + LibmdbxReader> BrontesBatchPricer<T, DB> {
    pub fn new(
        finished: Arc<AtomicBool>,
        quote_asset: Address,
        graph_manager: GraphManager<DB>,
        update_rx: UnboundedReceiver<DexPriceMsg>,
        provider: Arc<T>,
        current_block: u64,
        new_graph_pairs: HashMap<Address, (Protocol, Pair)>,
    ) -> Self {
        Self {
            finished,
            new_graph_pairs,
            quote_asset,
            buffer: StateBuffer::new(),
            update_rx,
            graph_manager,
            dex_quotes: HashMap::default(),
            lazy_loader: LazyExchangeLoader::new(provider),
            current_block,
            completed_block: current_block,
            overlap_update: None,
        }
    }

    /// Handles pool updates for the BrontesBatchPricer system.
    ///
    /// This function processes a vector of `PoolUpdate` messages, updating the
    /// current block tracking and incorporating new pools into the graph
    /// manager. It filters updates to identify and add new pools, using
    /// details such as address, protocol, and pair. The function also
    /// manages state transitions and pools, buffering state changes by
    /// block number and adding subgraphs for new pools if they don't
    /// already exist in the graph manager.
    ///
    /// Essentially, it ensures the graph manager remains synchronized with the
    /// latest block data, maintaining the integrity and accuracy of
    /// the decentralized exchange pricing mechanism.
    fn on_pool_updates(&mut self, updates: Vec<PoolUpdate>) {
        if updates.is_empty() {
            return
        };

        if let Some(msg) = updates.first() {
            if msg.block > self.current_block {
                self.current_block = msg.block;
            }
        }
        // insert new pools accessed on this block.
        updates
            .iter()
            .filter_map(|update| {
                let (protocol, pair) = self.new_graph_pairs.remove(&update.get_pool_address())?;
                Some((update.get_pool_address(), protocol, pair, update.block))
            })
            .for_each(|(pool_addr, protocol, pair, block)| {
                self.graph_manager
                    .add_pool(pair, pool_addr, protocol, block);
            });

        tracing::debug!("search triggered by on pool updates");
        let (state, pools) = graph_search_par(&self.graph_manager, self.quote_asset, updates);
        tracing::debug!("search triggered by on pool updates completed");

        state.into_iter().flatten().for_each(|(addr, update)| {
            let block = update.block;
            self.buffer
                .updates
                .entry(block)
                .or_default()
                .push_back((addr, update));
        });

        pools
            .into_iter()
            .flatten()
            .unique_by(|(_, p, _)| *p)
            .for_each(|(graph_edges, pair, block)| {
                if graph_edges.is_empty() {
                    error!(?pair, "new pool has no graph edges");
                    return
                }

                if self.graph_manager.has_subgraph(pair) {
                    error!(?pair, "already have pairs");
                    return
                }

                self.add_subgraph(pair, block, graph_edges, false);
            });
    }

    fn get_dex_price(&self, pool_pair: Pair) -> Option<Rational> {
        if pool_pair.0 == pool_pair.1 {
            return Some(Rational::ONE)
        }
        self.graph_manager.get_price(pool_pair)
    }

    /// For a given block number and tx idx, finds the path to the following
    /// tokens and inserts the data into dex_quotes.
    fn store_dex_price(&mut self, block: u64, tx_idx: u64, pool_pair: Pair, prices: DexPrices) {
        // insert the pool keys into the price map
        match self.dex_quotes.entry(block) {
            Entry::Occupied(mut quotes) => {
                let q = quotes.get_mut();
                let size = q.0.len();
                // pad the vector
                for _ in size..=tx_idx as usize {
                    q.0.push(None)
                }
                let tx = q.0.get_mut(tx_idx as usize).unwrap();

                if let Some(tx) = tx.as_mut() {
                    tx.insert(pool_pair, prices);
                } else {
                    let mut tx_pairs = HashMap::default();
                    tx_pairs.insert(pool_pair, prices);
                    *tx = Some(tx_pairs)
                }
            }
            Entry::Vacant(v) => {
                // pad the vec to the tx index
                let mut vec = Vec::new();
                for _ in 0..=tx_idx as usize {
                    vec.push(None);
                }
                // insert
                let mut map = HashMap::new();
                map.insert(pool_pair, prices);

                let entry = vec.get_mut(tx_idx as usize).unwrap();
                *entry = Some(map);

                v.insert(DexQuotes(vec));
            }
        }
    }

    /// Similar to update known state but doesn't apply the state transfer given
    /// the pool is from end of block.
    fn init_new_pool_override(&mut self, addr: Address, msg: PoolUpdate) {
        let tx_idx = msg.tx_idx;
        let block = msg.block;

        let Some(pool_pair) = msg.get_pair(self.quote_asset) else {
            error!(?addr, "failed to get pair for pool");
            return
        };

        // generate all variants of the price that might be used in the inspectors
        let pair0 = Pair(pool_pair.0, self.quote_asset);
        let pair1 = Pair(pool_pair.1, self.quote_asset);
        let Some(price0) = self.get_dex_price(pair0) else {
            error!(?pair0, "no price for token");
            return
        };

        let Some(price1) = self.get_dex_price(pair1) else {
            error!(?pair1, "no price for token");
            return
        };

        let price0 = DexPrices { post_state: price0.clone(), pre_state: price0 };
        let price1 = DexPrices { post_state: price1.clone(), pre_state: price1 };

        self.store_dex_price(block, tx_idx, pair0, price0);
        self.store_dex_price(block, tx_idx, pair1, price1);
    }

    fn update_known_state(&mut self, addr: Address, msg: PoolUpdate) {
        let tx_idx = msg.tx_idx;
        let block = msg.block;
        let Some(pool_pair) = msg.get_pair(self.quote_asset) else {
            error!(?addr, "failed to get pair for pool");
            return;
        };

        // add price post state
        let pair0 = Pair(pool_pair.0, self.quote_asset);
        let pair1 = Pair(pool_pair.1, self.quote_asset);

        let Some(price0_pre) = self.get_dex_price(pair0) else {
            error!(?pair0, "no price for token");
            return
        };
        let Some(price1_pre) = self.get_dex_price(pair1) else {
            error!(?pair1, "no price for token");
            return
        };
        self.graph_manager.update_state(addr, msg);

        let Some(price0_post) = self.get_dex_price(pair0) else {
            error!(?pair0, "no price for token");
            return
        };
        let Some(price1_post) = self.get_dex_price(pair1) else {
            error!(?pair1, "no price for token");
            return
        };

        self.store_dex_price(
            block,
            tx_idx,
            pair0,
            DexPrices { pre_state: price0_pre, post_state: price0_post },
        );

        self.store_dex_price(
            block,
            tx_idx,
            pair1,
            DexPrices { pre_state: price1_pre, post_state: price1_post },
        );
    }

    /// Processes the result of lazy pool state loading. It updates the graph
    /// state or handles errors.
    ///
    /// # Behavior
    /// If the pool state is successfully loaded, the function updates the graph
    /// manager with the new state. If the pool was initialized in the
    /// current block and the load result indicates an error, an override is set
    /// to prevent invalid state application. It then triggers subgraph
    /// verification for relevant pairs. In case of a load error, it handles
    /// the error by calling `on_state_load_error`.
    ///
    /// # Usage
    /// This function is used within the system to handle the outcomes of
    /// asynchronous pool state loading operations, ensuring the graph remains
    /// accurate and up-to-date.
    fn on_pool_resolve(&mut self, state: LazyResult) {
        let LazyResult { block, state, load_result } = state;

        if let Some(state) = state {
            let addr = state.address();

            self.graph_manager.new_state(addr, state);

            // pool was initialized this block. lets set the override to avoid invalid state
            if !load_result.is_ok() {
                self.buffer.overrides.entry(block).or_default().insert(addr);
            }
        } else if let LoadResult::Err { pool_address, pool_pair, protocol } = load_result {
            self.new_graph_pairs
                .insert(pool_address, (protocol, pool_pair));
        }
    }

    /// Attempts to verify subgraphs for a given set of pairs and handles the
    /// verification results.
    ///
    /// # Behavior
    /// The function triggers subgraph verification for each provided pair and
    /// block number combination. On successful verification, it prunes bad
    /// edges from the subgraph and updates the graph manager with the verified
    /// subgraph. If verification fails, it prunes bad edges and prepares
    /// the failed pair for requery. After processing the verification
    /// results, it requeues any pairs that need to be reverified due to failed
    /// verification.
    fn try_verify_subgraph(&mut self, pairs: Vec<(u64, Option<u64>, Pair)>) {
        let requery = self
            .graph_manager
            .verify_subgraph(pairs, self.quote_asset)
            .into_iter()
            .filter_map(|result| match result {
                VerificationResults::Passed(passed) => {
                    passed.prune_state.into_iter().for_each(|(_, bad_edges)| {
                        for bad_edge in bad_edges {
                            if let Some((addr, protocol, pair)) = self
                                .graph_manager
                                .remove_pair_graph_address(bad_edge.pair, bad_edge.pool_address)
                            {
                                self.new_graph_pairs.insert(addr, (protocol, pair));
                            }
                        }
                    });
                    self.graph_manager.add_verified_subgraph(
                        passed.pair,
                        passed.subgraph,
                        passed.block,
                    );

                    None
                }
                VerificationResults::Failed(failed) => {
                    failed.prune_state.into_iter().for_each(|(_, bad_edges)| {
                        for bad_edge in bad_edges {
                            if let Some((addr, protocol, pair)) = self
                                .graph_manager
                                .remove_pair_graph_address(bad_edge.pair, bad_edge.pool_address)
                            {
                                self.new_graph_pairs.insert(addr, (protocol, pair));
                            }
                        }
                    });

                    Some((failed.pair, failed.block, failed.ignore_state, failed.frayed_ends))
                }
            })
            .collect_vec();

        self.requery_bad_state_par(requery);
    }

    /// Requeries the state of subgraphs for given pairs that encountered issues
    /// during verification.
    ///
    /// # Behavior
    /// This function is invoked when subgraph verification fails for certain
    /// pairs. It reattempts to construct valid subgraphs by: 1. Requerying
    /// the state for each pair and block number, considering any ignored pairs
    /// during the graph construction. 2. Adding newly constructed subgraphs
    /// if they are non-empty, or recursively removing problematic pairs and
    /// requerying if necessary. 3. In cases where no valid paths are found
    /// after requery, it escalates the verification by analyzing alternative
    /// paths or pairs.
    fn requery_bad_state_par(&mut self, pairs: Vec<(Pair, u64, HashSet<Pair>, Vec<Address>)>) {
        if pairs.is_empty() {
            return
        }
        tracing::debug!("requerying bad state");

        let new_state = par_state_query(&self.graph_manager, pairs);
        if new_state.is_empty() {
            tracing::error!("requery bad state returned nothing");
        }

        let mut recusing = Vec::new();
        new_state
            .into_iter()
            .for_each(|(pair, block, missing_paths)| {
                let edges = missing_paths.into_iter().flatten().unique().collect_vec();
                // add regularly
                if edges.is_empty() {
                    self.rundown(pair, block);
                    return
                }

                let Some((id, need_state, force_rundown)) =
                    self.add_subgraph(pair, block, edges, true)
                else {
                    return;
                };

                if force_rundown {
                    self.rundown(pair, block);
                } else if !need_state {
                    recusing.push((block, id, pair))
                }
            });

        if !recusing.is_empty() {
            self.try_verify_subgraph(recusing);
        }
        tracing::debug!("finished requerying bad state");
    }

    /// rundown occurs when we have hit a attempt limit for trying to find high
    /// liquidity nodes for a pair subgraph. when this happens, we take all
    /// of the low liquidity nodes and generate all unique paths through each
    /// and then add it to the subgraph. And then allow for these low liquidity
    /// nodes as they are the only nodes for the given pair.
    fn rundown(&mut self, pair: Pair, block: u64) {
        let Some(ignores) = self.graph_manager.verify_subgraph_on_new_path_failure(pair) else {
            return
        };

        if ignores.is_empty() {
            tracing::error!(
                ?pair,
                ?block,
                "rundown for subgraph has no edges we are supposed to ignore"
            );
        }

        // take all combinations of our ignore nodes
        let queries = if ignores.len() > 1 {
            ignores
                .iter()
                .copied()
                .combinations(ignores.len() - 1)
                .map(|ignores| (pair, block, ignores.into_iter().collect::<HashSet<_>>(), vec![]))
                .collect_vec()
        } else {
            ignores
                .iter()
                .copied()
                .map(|_| (pair, block, HashSet::new(), vec![]))
                .collect_vec()
        };

        tracing::debug!(?pair, ?block, subgraph_variations = queries.len(), "starting rundown");

        let edges = par_state_query(&self.graph_manager, queries)
            .into_iter()
            .flat_map(|e| e.2)
            .flatten()
            .unique()
            .collect_vec();

        if edges.is_empty() {
            tracing::error!(?pair, ?block, "failed to find connection for graph");
            return
        } else {
            let Some((id, need_state, _)) = self.add_subgraph(pair, block, edges, true) else {
                return;
            };

            if !need_state {
                self.try_verify_subgraph(vec![(block, id, pair)]);
            }
        }
        tracing::debug!(?pair, ?block, "finished rundown");
    }

    /// Adds a subgraph for verification based on the given pair, block, and
    /// edges.
    ///
    /// # Behavior
    /// This function is responsible for initializing the process of verifying a
    /// new subgraph. It involves: 1. Adding the subgraph to the
    /// verification queue with the necessary edges and state. 2. Initiating
    /// lazy loading for the exchange pools involved in the subgraph if they are
    /// not already being loaded. 3. Adding the pool as a dependent to an
    /// ongoing load operation if it's already in progress.
    ///
    /// The function returns a boolean indicating whether any lazy loading was
    /// triggered during its execution. This function ensures that all necessary
    /// pool states are loaded and ready for accurate subgraph verification.
    fn add_subgraph(
        &mut self,
        pair: Pair,
        block: u64,
        edges: Vec<SubGraphEdge>,
        frayed_ext: bool,
    ) -> Option<(Option<u64>, bool, bool)> {
        let (needed_state, id, force_rundown) = if frayed_ext {
            let (need, id, force_rundown) = self
                .graph_manager
                .add_frayed_end_extension(pair, block, edges)?;
            (need, Some(id), force_rundown)
        } else {
            (
                self.graph_manager
                    .add_subgraph_for_verification(pair, block, edges),
                None,
                false,
            )
        };

        let mut triggered = false;
        // because we run these state fetches in parallel, we come across the issue
        // where in block N we have a path, it however doesn't get verified so we go to
        // query more state. however the new path it takes goes through a pool that is
        // being inited with state from block N + I, when we go to calculate the price
        // the state will be off thus giving us a incorrect price
        for pool_info in needed_state {
            let is_lazy_loading =
                if let Some(blocks) = self.lazy_loader.is_loading_block(&pool_info.pool_addr) {
                    blocks.contains(&block)
                } else {
                    false
                };

            if !is_lazy_loading {
                self.lazy_loader.lazy_load_exchange(
                    pair,
                    Pair(pool_info.token_0, pool_info.token_1),
                    id,
                    pool_info.pool_addr,
                    block,
                    pool_info.dex_type,
                );
                triggered = true;
            } else {
                self.lazy_loader
                    .add_protocol_parent(block, id, pool_info.pool_addr, pair);
                triggered = true;
            }
        }

        Some((id, triggered, force_rundown))
    }

    fn can_progress(&self) -> bool {
        self.lazy_loader.can_progress(&self.completed_block)
            && self.completed_block < self.current_block
    }

    /// allows for pre-processing of up to 4 future blocks
    /// before we only will focus on clearing current state
    fn process_future_blocks(&self) -> bool {
        self.completed_block + 5 > self.current_block
    }

    // called when we try to progress to the next block
    fn try_resolve_block(&mut self) -> Option<(u64, DexQuotes)> {
        // if there are still requests for the given block or the current block isn't
        // complete yet, then we wait
        if !self.can_progress() {
            return None
        }

        self.graph_manager.finalize_block(self.completed_block);

        // if all block requests are complete, lets apply all the state transitions we
        // had for the given block which will allow us to generate all pricing
        let (buffer, overrides) = (
            self.buffer
                .updates
                .remove(&self.completed_block)
                .unwrap_or_default(),
            self.buffer
                .overrides
                .remove(&self.completed_block)
                .unwrap_or_default(),
        );

        for (address, update) in buffer {
            if overrides.contains(&address) {
                // we will just init the pool but nothing else since the state of the pool is
                // end of block
                self.init_new_pool_override(address, update)
            } else {
                // make sure to apply state updates
                self.update_known_state(address, update);
            }
        }

        let block = self.completed_block;

        let res = self
            .dex_quotes
            .remove(&self.completed_block)
            .unwrap_or(DexQuotes(vec![]));

        self.completed_block += 1;

        // add new nodes to pair graph
        Some((block, res))
    }

    fn on_close(&mut self) -> Option<(u64, DexQuotes)> {
        if self.completed_block > self.current_block {
            return None
        }

        self.graph_manager.finalize_block(self.completed_block);

        // if all block requests are complete, lets apply all the state transitions we
        // had for the given block which will allow us to generate all pricing
        let (buffer, overrides) = (
            self.buffer
                .updates
                .remove(&self.completed_block)
                .unwrap_or_default(),
            self.buffer
                .overrides
                .remove(&self.completed_block)
                .unwrap_or_default(),
        );

        for (address, update) in buffer {
            if overrides.contains(&address) {
                self.init_new_pool_override(address, update)
            } else {
                self.update_known_state(address, update);
            }
        }

        let block = self.completed_block;

        let res = self
            .dex_quotes
            .remove(&self.completed_block)
            .unwrap_or(DexQuotes(vec![]));

        self.completed_block += 1;

        Some((block, res))
    }

    fn poll_state_processing(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Option<Poll<Option<(u64, DexQuotes)>>> {
        // because results tend to stack up, we always want to progress them first
        while let Poll::Ready(Some(state)) = self.lazy_loader.poll_next_unpin(cx) {
            self.on_pool_resolve(state)
        }

        let pairs = self.lazy_loader.pairs_to_verify();
        self.try_verify_subgraph(pairs);

        // check if we can progress to the next block.
        self.try_resolve_block()
            .map(|prices| Poll::Ready(Some(prices)))
    }
}

impl<T: TracingProvider, DB: LibmdbxReader + LibmdbxWriter + Unpin> Stream
    for BrontesBatchPricer<T, DB>
{
    type Item = (u64, DexQuotes);

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        // loop is very heavy, low amount of work needed
        let mut work = 128;
        loop {
            if let Some(new_prices) = self.poll_state_processing(cx) {
                return new_prices
            }

            if !self.process_future_blocks() {
                continue
            }

            let mut block_updates = Vec::new();
            loop {
                match self.update_rx.poll_recv(cx).map(|inner| {
                    inner.and_then(|action| match action {
                        DexPriceMsg::Update(update) => Some(PollResult::State(update)),
                        DexPriceMsg::DiscoveredPool(NormalizedPoolConfigUpdate {
                            protocol,
                            tokens,
                            pool_address,
                            ..
                        }) => {
                            if protocol.has_state_updater() {
                                self.new_graph_pairs
                                    .insert(pool_address, (protocol, Pair(tokens[0], tokens[1])));
                            };
                            Some(PollResult::DiscoveredPool)
                        }
                        DexPriceMsg::Closed => None,
                    })
                }) {
                    Poll::Ready(Some(u)) => {
                        if let PollResult::State(update) = u {
                            if let Some(overlap) = self.overlap_update.take() {
                                block_updates.push(overlap);
                            }
                            if update.block == self.current_block {
                                block_updates.push(update);
                            } else {
                                self.overlap_update = Some(update);
                                break
                            }
                        }
                    }
                    Poll::Ready(None) | Poll::Pending => {
                        if self.lazy_loader.is_empty()
                            && self.lazy_loader.can_progress(&self.completed_block)
                            && block_updates.is_empty()
                            && self.finished.load(SeqCst)
                        {
                            return Poll::Ready(self.on_close())
                        }
                        break
                    }
                }

                // we poll here to continuously progress state fetches as they are slow
                if let Poll::Ready(Some(state)) = self.lazy_loader.poll_next_unpin(cx) {
                    self.on_pool_resolve(state)
                }
            }

            self.on_pool_updates(block_updates);

            work -= 1;
            if work == 0 {
                cx.waker().wake_by_ref();
                return Poll::Pending
            }
        }
    }
}

enum PollResult {
    State(PoolUpdate),
    DiscoveredPool,
}

/// a ordered buffer for holding state transitions for a block while the lazy
/// loading of pools is being applied
pub struct StateBuffer {
    /// updates for a given block in order that they occur
    pub updates:   HashMap<u64, VecDeque<(Address, PoolUpdate)>>,
    /// when we have a override for a given address at a block. it means that
    /// we don't want to apply any pool updates for the block. This is useful
    /// for when a pool is initted at a block and we can only query the end
    /// of block state. we can override all pool updates for the init block
    /// to ensure our pool state is in sync
    pub overrides: HashMap<u64, HashSet<Address>>,
}

impl Default for StateBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl StateBuffer {
    pub fn new() -> Self {
        Self { updates: HashMap::default(), overrides: HashMap::default() }
    }
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

type GraphSeachParRes = (Vec<Vec<(Address, PoolUpdate)>>, Vec<Vec<(Vec<SubGraphEdge>, Pair, u64)>>);

fn graph_search_par<DB: LibmdbxWriter + LibmdbxReader>(
    graph: &GraphManager<DB>,
    quote: Address,
    updates: Vec<PoolUpdate>,
) -> GraphSeachParRes {
    let (state, pools): (Vec<_>, Vec<_>) = updates
        .into_par_iter()
        .filter_map(|msg| {
            let pair = msg.get_pair(quote)?;
            let pair0 = Pair(pair.0, quote);
            let pair1 = Pair(pair.1, quote);

            let (state, path) = on_new_pool_pair(
                graph,
                msg,
                (!graph.has_subgraph(pair0)).then_some(pair0),
                (!graph.has_subgraph(pair1)).then_some(pair1),
            );
            Some((state, path))
        })
        .unzip();

    (state, pools)
}

type ParStateQueryRes = Vec<(Pair, u64, Vec<Vec<SubGraphEdge>>)>;

fn par_state_query<DB: LibmdbxWriter + LibmdbxReader>(
    graph: &GraphManager<DB>,
    pairs: Vec<(Pair, u64, HashSet<Pair>, Vec<Address>)>,
) -> ParStateQueryRes {
    pairs
        .into_par_iter()
        .map(|(pair, block, ignore, frayed_ends)| {
            if frayed_ends.is_empty() {
                return (pair, block, vec![graph.create_subgraph(block, pair, ignore, 100, 3)])
            }
            (
                pair,
                block,
                frayed_ends
                    .into_iter()
                    .zip(vec![pair.0].into_iter().cycle())
                    .collect_vec()
                    .into_par_iter()
                    .map(|(end, start)| {
                        graph.create_subgraph(block, Pair(start, end), ignore.clone(), 0, 12)
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>()
}

type NewPoolPair = (Vec<(Address, PoolUpdate)>, Vec<(Vec<SubGraphEdge>, Pair, u64)>);

fn on_new_pool_pair<DB: LibmdbxWriter + LibmdbxReader>(
    graph: &GraphManager<DB>,
    msg: PoolUpdate,
    pair0: Option<Pair>,
    pair1: Option<Pair>,
) -> NewPoolPair {
    let block = msg.block;

    let mut buf_pending = Vec::new();
    let mut path_pending = Vec::new();

    // add default pair to buffer to make sure that we price all pairs and apply the
    // state diff. we don't wan't to actually do a graph search for this pair
    // though.
    buf_pending.push((msg.get_pool_address(), msg.clone()));

    // we add support for fetching the pair as well as each individual token with
    // the given quote asset
    let mut trigger_update = msg;
    // we want to make sure no updates occur to the state of the virtual pool when
    // we load it
    trigger_update.logs = vec![];

    // add first pair
    if let Some(pair0) = pair0 {
        trigger_update.action = make_fake_swap(pair0);
        if let Some((buf, path)) =
            queue_loading_returns(graph, block, pair0, trigger_update.clone())
        {
            buf_pending.push(buf);
            path_pending.push(path);
        }
    }

    // add second direction
    if let Some(pair1) = pair1 {
        trigger_update.action = make_fake_swap(pair1);
        if let Some((buf, path)) =
            queue_loading_returns(graph, block, pair1, trigger_update.clone())
        {
            buf_pending.push(buf);
            path_pending.push(path);
        }
    }

    (buf_pending, path_pending)
}

type LoadingReturns = Option<((Address, PoolUpdate), (Vec<SubGraphEdge>, Pair, u64))>;

fn queue_loading_returns<DB: LibmdbxWriter + LibmdbxReader>(
    graph: &GraphManager<DB>,
    block: u64,
    pair: Pair,
    trigger_update: PoolUpdate,
) -> LoadingReturns {
    if pair.0 == pair.1 {
        return None
    }

    Some(((trigger_update.get_pool_address(), trigger_update.clone()), {
        let subgraph = graph.create_subgraph(block, pair, HashSet::new(), 100, 5);
        (subgraph, pair, trigger_update.block)
    }))
}

#[cfg(feature = "testing")]
impl<T: TracingProvider, DB: LibmdbxWriter + LibmdbxReader> BrontesBatchPricer<T, DB> {
    pub fn get_lazy_loader(&mut self) -> &mut LazyExchangeLoader<T> {
        &mut self.lazy_loader
    }

    pub fn get_buffer(&mut self) -> &mut StateBuffer {
        &mut self.buffer
    }
}

#[cfg(test)]
pub mod test {

    use alloy_primitives::{hex, Address, FixedBytes};

    pub const USDC_ADDRESS: Address =
        Address(FixedBytes::<20>(hex!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")));
}
