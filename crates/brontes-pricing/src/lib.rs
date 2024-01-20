#![allow(unused)]
#![feature(noop_waker)]
pub mod exchanges;
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
    exchanges::StaticBindingsDb,
    extra_processing::Pair,
    normalized_actions::{Actions, NormalizedAction, NormalizedSwap},
    traits::TracingProvider,
};
use ethers::core::k256::elliptic_curve::bigint::Zero;
use exchanges::lazy::{LazyExchangeLoader, LazyResult, LoadResult};
pub use exchanges::*;
pub use graphs::{AllPairGraph, GraphManager, SubGraphEdge};
use rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};

mod graphs;

use futures::{Future, Stream, StreamExt};
use graphs::{PoolPairInfoDirection, PoolPairInformation};
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::{debug, error, info, warn};
use types::{DexPriceMsg, DexQuotes, DiscoveredPool, PoolUpdate};

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
    new_graph_pairs: HashMap<Address, (StaticBindingsDb, Pair)>,
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
        max_pool_loading_tasks: usize,
        quote_asset: Address,
        graph_manager: GraphManager,
        update_rx: UnboundedReceiver<DexPriceMsg>,
        provider: Arc<T>,
        current_block: u64,
        new_graph_pairs: HashMap<Address, (StaticBindingsDb, Pair)>,
    ) -> Self {
        Self {
            new_graph_pairs,
            quote_asset,
            buffer: StateBuffer::new(),
            update_rx,
            graph_manager,
            dex_quotes: HashMap::default(),
            lazy_loader: LazyExchangeLoader::new(provider, max_pool_loading_tasks),
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

        // only add a new pool to the graph when we have a update for it. this will help
        // us avoid dead pools in the graph;
        let new_pools = updates
            .iter()
            .filter_map(|update| {
                let (protocol, pair) = self.new_graph_pairs.remove(&update.get_pool_address())?;
                Some((update.get_pool_address(), protocol, pair))
            })
            .for_each(|(pool_addr, protocol, pair)| {
                self.graph_manager.add_pool(pair, pool_addr, protocol);
            });

        let (state, pools) =
            graph_search_par(&self.graph_manager, self.quote_asset, self.current_block, updates);

        state
            .into_iter()
            .filter_map(|s| s)
            .flatten()
            .for_each(|(addr, update)| {
                let block = update.block;
                self.buffer
                    .updates
                    .entry(block)
                    .or_default()
                    .push_back((addr, update));
            });

        pools
            .into_iter()
            .filter_map(|s| s)
            .flatten()
            .for_each(|(pool_infos, graph_edges, pair)| {
                if graph_edges.is_empty() {
                    return
                }
                for pool_info in pool_infos {
                    let lazy_loading = self.lazy_loader.is_loading(&pool_info.pool_addr);
                    // load exchange only if its not loaded already
                    if !(self.graph_manager.has_state(&pool_info.pool_addr) || lazy_loading) {
                        self.lazy_loader.lazy_load_exchange(
                            pair,
                            Pair(pool_info.token_0, pool_info.token_1),
                            pool_info.pool_addr,
                            self.current_block,
                            pool_info.dex_type,
                        );
                    } else if lazy_loading {
                        self.lazy_loader
                            .add_protocol_parent(pool_info.pool_addr, pair);
                    }
                }

                self.graph_manager.add_subgraph(pair, graph_edges);
            })
    }

    /// because we already have a state update for this pair in the buffer, we
    /// don't wanna create another one
    fn re_queue_bad_pair(&mut self, pair: Pair, block: u64) {
        if pair.0 == pair.1 {
            return
        }

        for pool_info in self.graph_manager.create_subpool(block, pair).into_iter() {
            let is_loading = self.lazy_loader.is_loading(&pool_info.pool_addr);
            // load exchange only if its not loaded already
            if !(self.graph_manager.has_state(&pool_info.pool_addr) || is_loading) {
                self.lazy_loader.lazy_load_exchange(
                    pair,
                    Pair(pool_info.token_0, pool_info.token_1),
                    pool_info.pool_addr,
                    block,
                    pool_info.dex_type,
                );
            } else if is_loading {
                self.lazy_loader
                    .add_protocol_parent(pool_info.pool_addr, pair)
            }
        }
    }

    /// For a given block number and tx idx, finds the path to the following
    /// tokens and inserts the data into dex_quotes.
    fn store_dex_price(&mut self, block: u64, tx_idx: u64, pool_pair: Pair) {
        if pool_pair.0 == pool_pair.1 {
            return
        }

        // query graph for all keys needed to properly query price for a given pair
        let Some(price) = self.graph_manager.get_price(pool_pair) else {
            error!(?pool_pair, "no price from graph manager");
            return
        };

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
                    tx.insert(pool_pair, price);
                } else {
                    let mut tx_pairs = HashMap::default();
                    tx_pairs.insert(pool_pair, price);
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
                map.insert(pool_pair, price);

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

        self.store_dex_price(block, tx_idx, pair0);
        self.store_dex_price(block, tx_idx, pair1);
    }

    fn update_known_state(&mut self, addr: Address, msg: PoolUpdate) {
        let tx_idx = msg.tx_idx;
        let block = msg.block;
        let Some(pool_pair) = msg.get_pair(self.quote_asset) else {
            error!(?addr, "failed to get pair for pool");
            return;
        };
        self.graph_manager.update_state(addr, msg);

        // generate all variants of the price that might be used in the inspectors
        let pair0 = Pair(pool_pair.0, self.quote_asset);
        let pair1 = Pair(pool_pair.1, self.quote_asset);

        self.store_dex_price(block, tx_idx, pair0);
        self.store_dex_price(block, tx_idx, pair1);
    }

    fn can_progress(&self) -> bool {
        self.lazy_loader.requests_for_block(&self.completed_block) == 0
            && self.completed_block < self.current_block
    }

    // called when we try to progress to the next block
    fn try_resolve_block(&mut self) -> Option<(u64, DexQuotes)> {
        // if there are still requests for the given block or the current block isn't
        // complete yet, then we wait
        if !self.can_progress() {
            return None
        }

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

        info!(
            block_number = self.completed_block,
            dex_quotes_length = res.0.len(),
            "got dex quotes"
        );

        self.completed_block += 1;

        // add new nodes to pair graph
        Some((block, res))
    }

    fn on_pool_resolve(&mut self, state: LazyResult) {
        let LazyResult { block, state, load_result } = state;

        if let Some(state) = state {
            let addr = state.address();

            self.graph_manager
                .new_state(self.completed_block, addr, state);

            // pool was initialized this block. lets set the override to avoid invalid state
            if !load_result.is_ok() {
                self.buffer.overrides.entry(block).or_default().insert(addr);
            }
        } else if let LoadResult::Err { pool_address, pool_pair, block } = load_result {
            self.lazy_loader
                .remove_protocol_parents(&pool_address)
                .into_iter()
                .for_each(|parent_pair| {
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
    }

    fn on_close(&mut self) -> Option<(u64, DexQuotes)> {
        if self.completed_block >= self.current_block + 1 {
            return None
        }

        info!(?self.completed_block,"getting ready to calc dex prices");
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

        info!(dex_quotes = res.0.len(), "got dex quotes");

        self.completed_block += 1;

        Some((block, res))
    }

    fn poll_state_processing(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Option<Poll<Option<(u64, DexQuotes)>>> {
        // because results tend to stack up, we always want to progress them first
        let mut work = 256;
        loop {
            if let Poll::Ready(Some(state)) = self.lazy_loader.poll_next_unpin(cx) {
                self.on_pool_resolve(state)
            }

            // check if we can progress to the next block.
            let block_prices = self.try_resolve_block();
            if block_prices.is_some() {
                return Some(Poll::Ready(block_prices))
            }

            work -= 1;
            if work == 0 {
                break
            }
        }
        None
    }
}

impl<T: TracingProvider> Stream for BrontesBatchPricer<T> {
    type Item = (u64, DexQuotes);

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
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
                        block,
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
                    if self.lazy_loader.is_empty() && block_updates.is_empty() {
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

        cx.waker().wake_by_ref();
        return Poll::Pending
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

pub struct AllPairFindRequests {
    updates: Vec<PoolUpdate>,
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
    block: u64,
    updates: Vec<PoolUpdate>,
) -> (
    Vec<Option<Vec<(Address, PoolUpdate)>>>,
    Vec<Option<Vec<(Vec<PoolPairInfoDirection>, Vec<SubGraphEdge>, Pair)>>>,
) {
    let (state, pools): (Vec<_>, Vec<_>) = updates
        .into_par_iter()
        .map(|msg| {
            let pair = msg.get_pair(quote).unwrap();
            if graph.has_subgraph(pair) {
                let addr = msg.get_pool_address();
                (Some(vec![(addr, msg)]), None)
            } else {
                let (state, path) = on_new_pool_pair(graph, quote, block, msg);
                (Some(state), Some(path))
            }
        })
        .unzip();

    (state, pools)
}
fn on_new_pool_pair(
    graph: &GraphManager,
    quote: Address,
    block: u64,
    msg: PoolUpdate,
) -> (Vec<(Address, PoolUpdate)>, Vec<(Vec<PoolPairInfoDirection>, Vec<SubGraphEdge>, Pair)>) {
    let pair = msg.get_pair(quote).unwrap();

    let mut buf_pending = Vec::new();
    let mut path_pending = Vec::new();
    // add pool pair
    if let Some((buf, path)) = queue_loading_returns(graph, block, pair, msg.clone()) {
        buf_pending.push(buf);
        path_pending.push(path);
    }

    // we add support for fetching the pair as well as each individual token with
    // the given quote asset
    let mut trigger_update = msg;
    // we want to make sure no updates occur to the state of the virtual pool when
    // we load it
    trigger_update.logs = vec![];

    // add first pair
    let pair0 = Pair(pair.0, quote);
    trigger_update.action = make_fake_swap(pair0);
    if let Some((buf, path)) = queue_loading_returns(graph, block, pair0, trigger_update.clone()) {
        buf_pending.push(buf);
        path_pending.push(path);
    }

    // add second direction
    let pair1 = Pair(pair.1, quote);
    trigger_update.action = make_fake_swap(pair1);

    if let Some((buf, path)) = queue_loading_returns(graph, block, pair1, trigger_update.clone()) {
        buf_pending.push(buf);
        path_pending.push(path);
    }

    (buf_pending, path_pending)
}

fn queue_loading_returns(
    graph: &GraphManager,
    block: u64,
    pair: Pair,
    trigger_update: PoolUpdate,
) -> Option<((Address, PoolUpdate), (Vec<PoolPairInfoDirection>, Vec<SubGraphEdge>, Pair))> {
    if pair.0 == pair.1 {
        return None
    }

    Some(((trigger_update.get_pool_address(), trigger_update.clone()), {
        let (state, subgraph) = graph.crate_subpool_multithread(block, pair);
        (state, subgraph, pair)
    }))
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
    use brontes_pricing::test_utils::PricingTestUtils;

    pub const USDC_ADDRESS: Address =
        Address(FixedBytes::<20>(hex!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")));

    #[tokio::test]
    #[serial]
    pub async fn test_result_building() {
        let block = 18500648;
        // errors on 0x56c03b8c4fa80ba37f5a7b60caaaef749bb5b220
        let pricing_utils = PricingTestUtils::new(USDC_ADDRESS);

        let (mut dex_pricer, mut tree) = pricing_utils
            .setup_dex_pricer_for_block(block)
            .await
            .unwrap();

        let noop_waker = Waker::noop();
        let mut cx = Context::from_waker(&noop_waker);

        // query all of the state we need to load into the lazy loader
        {
            let pinned = Pin::new(&mut dex_pricer);
            let res = pinned.poll_next(&mut cx);
            assert!(res.is_pending());
        }

        let missing_pricing_addr = Address(hex!("56c03b8c4fa80ba37f5a7b60caaaef749bb5b220").into());
        let missing_pair: Pair(missing_pricing_addr, USDC_ADDRESS);

        let updates = dex_pricer.buffer.updates.get(&block).unwrap();

        assert!(updates.iter().any(|(_, update)| {
            let pair = update.get_pair(USDC_ADDRESS).unwrap();
            pair == missing_pair
        }));
    }
}
