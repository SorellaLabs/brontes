pub mod protocols;
pub mod types;

#[cfg(test)]
pub mod test_utils;

use std::{
    collections::{hash_map::Entry, HashMap, HashSet, VecDeque},
    sync::Arc,
    task::{Context, Poll},
};

use alloy_primitives::{Address, U256};
use brontes_types::{
    db::dex::DexPrices,
    normalized_actions::{Actions, NormalizedSwap},
    pair::Pair,
    traits::TracingProvider,
};
pub use graphs::{AllPairGraph, GraphManager, VerificationResults};
use itertools::Itertools;
use malachite::{num::basic::traits::One, Rational};
pub use price_graph_types::{
    PoolPairInfoDirection, PoolPairInformation, SubGraphEdge, SubGraphsEntry,
};
use protocols::lazy::{LazyExchangeLoader, LazyResult, LoadResult};
pub use protocols::{Protocol, *};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

mod graphs;
mod price_graph_types;

use brontes_types::db::dex::DexQuotes;
use futures::{Stream, StreamExt};
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::{error, info};
use types::{DexPriceMsg, DiscoveredPool, PoolUpdate};

use crate::types::PoolState;

/// # Brontes Batch Pricer
/// ## Reasoning
/// We create a token graph in order to provide the price of any
/// token in a wanted quote token. This allows us to see delta between
/// centralized and decentralized prices which allows us to classify
///
/// ## Implementation
/// The Brontes Batch pricer runs on a block by block basis, This process is as
/// followed:
///
/// 1) On a new highest block received from the update channel. All new pools
/// are added to the token graph as there are now valid paths.
///
/// 2) All new pools touched are loaded by the lazy loader.
///
/// 3) State transitions on all pools are put into the state buffer.
///
/// 4) Once lazy loading for the block is complete, all state transitions are
/// applied in order, when a transition is applied, the price is added into the
/// state map.
///
/// 5) Once state transitions are all applied and we have our formatted data.
/// The data is returned and the pricer continues onto the next block.
pub struct BrontesBatchPricer<T: TracingProvider> {
    quote_asset:     Address,
    current_block:   u64,
    completed_block: u64,

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
    graph_manager:   GraphManager,
    /// lazy loads dex pairs so we only fetch init state that is needed
    lazy_loader:     LazyExchangeLoader<T>,
    dex_quotes:      HashMap<u64, DexQuotes>,
    /// when we are pulling from the channel, because its not peekable we always
    /// pull out one more than we want. this acts as a cache for it
    overlap_update:  Option<PoolUpdate>,
}

impl<T: TracingProvider> BrontesBatchPricer<T> {
    pub fn new(
        quote_asset: Address,
        graph_manager: GraphManager,
        update_rx: UnboundedReceiver<DexPriceMsg>,
        provider: Arc<T>,
        current_block: u64,
        new_graph_pairs: HashMap<Address, (Protocol, Pair)>,
    ) -> Self {
        Self {
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

        let (state, pools) = graph_search_par(&self.graph_manager, self.quote_asset, updates);

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

                self.add_subgraph(pair, block, graph_edges);
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

    fn can_progress(&self) -> bool {
        self.lazy_loader.can_progress(&self.completed_block)
            && self.completed_block < self.current_block
    }

    fn on_pool_resolve(&mut self, state: LazyResult) {
        let LazyResult { block, state, load_result } = state;

        if let Some(state) = state {
            let addr = state.address();

            self.graph_manager.new_state(addr, state);

            // pool was initialized this block. lets set the override to avoid invalid state
            if !load_result.is_ok() {
                self.buffer.overrides.entry(block).or_default().insert(addr);
            }

            let pairs = self.lazy_loader.pairs_to_verify();
            self.try_verify_subgraph(pairs);
        } else if let LoadResult::Err { pool_address, pool_pair, block, dependent_pairs } =
            load_result
        {
            self.on_state_load_error(pool_pair, pool_address, block, dependent_pairs);
        }
    }

    fn on_state_load_error(
        &mut self,
        pool_pair: Pair,
        pool_address: Address,
        block: u64,
        dependent_pairs: Vec<Pair>,
    ) {
        dependent_pairs.into_iter().for_each(|parent_pair| {
            let (re_query, bad_state) =
                self.graph_manager
                    .bad_pool_state(parent_pair, pool_pair, pool_address);

            if re_query {
                self.re_queue_bad_pair(parent_pair, block);
            }
            if let Some((address, protocol, pair)) = bad_state {
                self.new_graph_pairs.insert(address, (protocol, pair));
            }
        });
    }

    fn try_verify_subgraph(&mut self, pairs: Vec<(u64, Pair)>) {
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
                    self.graph_manager
                        .add_verified_subgraph(passed.pair, passed.subgraph);

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

                    Some((failed.pair, failed.block, failed.ignore_state))
                }
            })
            .collect_vec();

        self.requery_bad_state_par(requery);
    }

    fn requery_bad_state_par(&mut self, pairs: Vec<(Pair, u64, HashSet<Pair>)>) {
        if pairs.is_empty() {
            return
        }

        let new_state = par_state_query(&self.graph_manager, pairs);
        if new_state.is_empty() {
            tracing::error!("requery bad state returned nothing");
        }

        new_state.into_iter().for_each(|(pair, block, edges)| {
            // add regularly
            if !edges.is_empty() {
                if !self.add_subgraph(pair, block, edges) {
                    info!(?pair, "recusing has edges");
                    self.try_verify_subgraph(vec![(block, pair)]);
                }
                return
            }

            let Some(mut ignores) = self.graph_manager.verify_subgraph_on_new_path_failure(pair)
            else {
                error!(?pair, "failed to build a graph without any previous state removal");
                return
            };

            loop {
                let popped = ignores.pop();
                let (pair, block, edges) = par_state_query(
                    &self.graph_manager,
                    vec![(pair, block, ignores.iter().copied().collect())],
                )
                .remove(0);

                if edges.is_empty() {
                    if popped.is_none() {
                        break
                    }
                    continue
                } else {
                    if !self.add_subgraph(pair, block, edges) {
                        info!(?pair, "recusing on new path failures");
                        self.try_verify_subgraph(vec![(block, pair)]);
                    }

                    return
                }
            }
        });
    }

    fn add_subgraph(&mut self, pair: Pair, block: u64, edges: Vec<SubGraphEdge>) -> bool {
        let needed_state = self
            .graph_manager
            .add_subgraph_for_verification(pair, block, edges);

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
                    pool_info.pool_addr,
                    block,
                    pool_info.dex_type,
                );
                triggered = true;
            } else {
                self.lazy_loader
                    .add_protocol_parent(block, pool_info.pool_addr, pair);
                triggered = true;
            }
        }

        triggered
    }

    /// because we already have a state update for this pair in the buffer, we
    /// don't wanna create another one
    fn re_queue_bad_pair(&mut self, pair: Pair, block: u64) {
        if pair.0 == pair.1 {
            return
        }

        for pool_info in self
            .graph_manager
            .create_subgraph_mut(block, pair)
            .into_iter()
        {
            let is_loading = self.lazy_loader.is_loading(&pool_info.pool_addr);
            // load exchange only if its not loaded already
            if is_loading {
                self.lazy_loader
                    .add_protocol_parent(block, pool_info.pool_addr, pair)
            } else {
                self.lazy_loader.lazy_load_exchange(
                    pair,
                    Pair(pool_info.token_0, pool_info.token_1),
                    pool_info.pool_addr,
                    block,
                    pool_info.dex_type,
                )
            }
        }
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
        if self.completed_block >= self.current_block + 1 {
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

        // check if we can progress to the next block.
        self.try_resolve_block()
            .map(|prices| Poll::Ready(Some(prices)))
    }
}

impl<T: TracingProvider> Stream for BrontesBatchPricer<T> {
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

            let mut block_updates = Vec::new();
            loop {
                match self.update_rx.poll_recv(cx).map(|inner| {
                    inner.and_then(|action| match action {
                        DexPriceMsg::Update(update) => Some(PollResult::State(update)),
                        DexPriceMsg::DiscoveredPool(
                            DiscoveredPool { protocol, tokens, pool_address },
                            _block,
                        ) => {
                            if tokens.len() == 2 {
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
                    Poll::Ready(None) => {
                        if self.lazy_loader.is_empty()
                            && self.lazy_loader.can_progress(&self.completed_block)
                            && block_updates.is_empty()
                        {
                            return Poll::Ready(self.on_close())
                        } else {
                            break
                        }
                    }
                    Poll::Pending => break,
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
    Actions::Swap(NormalizedSwap {
        trace_index: 0,
        from:        Address::ZERO,
        recipient:   Address::ZERO,
        pool:        Address::ZERO,
        token_in:    pair.0,
        token_out:   pair.1,
        amount_in:   U256::ZERO,
        amount_out:  U256::ZERO,
    })
}

fn graph_search_par(
    graph: &GraphManager,
    quote: Address,
    updates: Vec<PoolUpdate>,
) -> (Vec<Vec<(Address, PoolUpdate)>>, Vec<Vec<(Vec<SubGraphEdge>, Pair, u64)>>) {
    let (state, pools): (Vec<_>, Vec<_>) = updates
        .into_par_iter()
        .map(|msg| {
            let pair = msg.get_pair(quote).unwrap();
            let pair0 = Pair(pair.0, quote).ordered();
            let pair1 = Pair(pair.1, quote).ordered();

            let (state, path) = on_new_pool_pair(
                graph,
                msg,
                (!graph.has_subgraph(pair0)).then_some(pair0),
                (!graph.has_subgraph(pair1)).then_some(pair1),
            );
            (state, path)
        })
        .unzip();

    (state, pools)
}

fn par_state_query(
    graph: &GraphManager,
    pairs: Vec<(Pair, u64, HashSet<Pair>)>,
) -> Vec<(Pair, u64, Vec<SubGraphEdge>)> {
    pairs
        .into_par_iter()
        .map(|(pair, block, ignore)| {
            let edge = graph.create_subgraph(block, pair, ignore);
            (pair, block, edge)
        })
        .collect::<Vec<_>>()
}

fn on_new_pool_pair(
    graph: &GraphManager,
    msg: PoolUpdate,
    pair0: Option<Pair>,
    pair1: Option<Pair>,
) -> (Vec<(Address, PoolUpdate)>, Vec<(Vec<SubGraphEdge>, Pair, u64)>) {
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

fn queue_loading_returns(
    graph: &GraphManager,
    block: u64,
    pair: Pair,
    trigger_update: PoolUpdate,
) -> Option<((Address, PoolUpdate), (Vec<SubGraphEdge>, Pair, u64))> {
    if pair.0 == pair.1 {
        return None
    }

    Some(((trigger_update.get_pool_address(), trigger_update.clone()), {
        let subgraph = graph.create_subgraph(block, pair, HashSet::new());
        (subgraph, pair, trigger_update.block)
    }))
}

#[cfg(feature = "testing")]
impl<T: TracingProvider> BrontesBatchPricer<T> {
    pub fn get_lazy_loader(&mut self) -> &mut LazyExchangeLoader<T> {
        &mut self.lazy_loader
    }

    pub fn get_buffer(&mut self) -> &mut StateBuffer {
        &mut self.buffer
    }
}

#[cfg(test)]
pub mod test {

    use std::{
        pin::Pin,
        task::{RawWaker, RawWakerVTable, Waker},
    };

    use alloy_primitives::{hex, Address, FixedBytes};
    use futures::future::poll_fn;
    use serial_test::serial;

    use super::*;
    use crate::test_utils::PricingTestUtils;

    pub const USDC_ADDRESS: Address =
        Address(FixedBytes::<20>(hex!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")));
}
