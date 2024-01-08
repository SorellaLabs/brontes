#![allow(unused)]
pub mod exchanges;
mod graph;
pub mod types;
use std::{
    collections::{hash_map::Entry, HashMap, HashSet, VecDeque},
    sync::Arc,
    task::Poll,
};

use alloy_primitives::{Address, U256};
use brontes_types::{
    exchanges::StaticBindingsDb,
    extra_processing::Pair,
    normalized_actions::{Actions, NormalizedAction, NormalizedSwap},
    traits::TracingProvider,
};
use ethers::core::k256::elliptic_curve::bigint::Zero;
use exchanges::lazy::{LazyExchangeLoader, LazyResult};
pub use exchanges::*;
use futures::{Future, Stream, StreamExt};
pub use graph::PairGraph;
use graph::{PoolPairInfoDirection, PoolPairInformation};
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::{debug, error, info, warn};
use types::{
    DexPriceMsg, DexPrices, DexQuotes, PoolKeyWithDirection, PoolStateSnapShot, PoolUpdate,
};

use crate::types::{PoolKey, PoolKeysForPair, PoolState};

/// # Brontes Batch Pricer
/// ## Reasoning
/// uses a token graph in order to provide the price of any
/// token in a wanted quote token. A token graph is used here so that we can
/// keep our pricing strictly to DEFI. This allows us to see delta between
/// centralized and decentralized prices which allows us to classify
///
/// ## Implimentation
/// The Brontes Batch pricer runs on a block by block basis, This process is as
/// followed:
///
/// 1) On a new highest block recieved from the update channel. All new pools
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
    quote_asset: Address,
    run:         u64,
    batch_id:    u64,

    current_block:   u64,
    completed_block: u64,

    /// receiver from classifier, classifier is ran sequential to grantee order
    update_rx:       UnboundedReceiver<DexPriceMsg>,
    /// holds the state transfers and state void overrides for the given block.
    /// how this works is that we process all state transitions for a block and
    /// allow lazy loading to occur. Once lazy loading has occurred and there
    /// are no more events for the current block, all the state transitions
    /// are applied in order with the price at the transaction index being
    /// calculated and inserted into the results and returned.
    buffer:          StateBuffer,
    /// holds new graph nodes / edges that can be added at every given block.
    /// this is done to ensure any route from a base to our quote asset will
    /// only pass though valid created pools.
    new_graph_pairs: HashMap<u64, Vec<(Address, StaticBindingsDb, Pair)>>,

    /// holds all token pairs for the given chunk.
    pair_graph:      PairGraph,
    /// lazy loads dex pairs so we only fetch init state that is needed
    lazy_loader:     LazyExchangeLoader<T>,
    /// mutable version of the pool. used for producing deltas
    mut_state:       HashMap<Address, PoolState>,
    /// tracks the last updated key for the given pool
    last_update:     HashMap<Address, PoolKey>,
    /// quotes
    dex_quotes:      HashMap<u64, DexQuotes>,
    /// the pool-key with finalized state
    finalized_state: HashMap<PoolKey, PoolStateSnapShot>,
}

impl<T: TracingProvider> BrontesBatchPricer<T> {
    pub fn new(
        quote_asset: Address,
        run: u64,
        batch_id: u64,
        pair_graph: PairGraph,
        update_rx: UnboundedReceiver<DexPriceMsg>,
        provider: Arc<T>,
        current_block: u64,
        new_graph_pairs: HashMap<u64, Vec<(Address, StaticBindingsDb, Pair)>>,
    ) -> Self {
        Self {
            new_graph_pairs,
            quote_asset,
            buffer: StateBuffer::new(),
            run,
            batch_id,
            update_rx,
            pair_graph,
            finalized_state: HashMap::default(),
            dex_quotes: HashMap::default(),
            lazy_loader: LazyExchangeLoader::new(provider),
            mut_state: HashMap::default(),
            last_update: HashMap::default(),
            current_block,
            completed_block: current_block,
        }
    }

    /// Handles new updates from the classifier, these will always be in order
    fn on_message(&mut self, msg: PoolUpdate) {
        if msg.block > self.current_block {
            self.current_block = msg.block;

            for (pool_addr, dex, pair) in self
                .new_graph_pairs
                .remove(&self.current_block)
                .unwrap_or_default()
            {
                self.pair_graph.add_node(pair, pool_addr, dex);
            }
        }

        let addr = msg.get_pool_address();

        // if we already have the state, we want to buffer the update to allow for all
        // init fetches to be done so that we can then go through and apply all
        // price transitions correctly to ensure order
        if self.mut_state.contains_key(&addr) || self.lazy_loader.is_loading(&addr) {
            self.buffer
                .updates
                .entry(msg.block)
                .or_default()
                .push_back((addr, msg));
        } else {
            // the update for the pool is buffered at queue loading
            self.on_new_pool(msg)
        }
    }

    /// takes the given pair fetching all pool state that needs to be loaded in
    /// order to properly price through the graph
    fn queue_loading(&mut self, pair: Pair, trigger_update: PoolUpdate) {
        for pool_info in self.pair_graph.get_path(pair).flatten() {
            // load exchange only if its not loaded already
            if !(self.mut_state.contains_key(&pool_info.info.pool_addr)
                || self.lazy_loader.is_loading(&pool_info.info.pool_addr))
            {
                self.lazy_loader.lazy_load_exchange(
                    pool_info.info.pool_addr,
                    trigger_update.block,
                    pool_info.info.dex_type,
                );
            }

            // we buffer the update for all of the pool state with there specific addresses
            self.buffer
                .updates
                .entry(trigger_update.block)
                .or_default()
                .push_back((pool_info.info.pool_addr, trigger_update.clone()));
        }
    }

    /// Called when we don't have the state for a given pool. starts the
    /// lazy load
    fn on_new_pool(&mut self, msg: PoolUpdate) {
        let Some(pair) = msg.get_pair(self.quote_asset) else {
            warn!(pool_update=?msg, "was not able to derive pair from update");
            return
        };

        // add pool pair
        self.queue_loading(pair, msg.clone());
        // flipped pool pair
        self.queue_loading(pair.flip(), msg.clone());

        // we add support for fetching the pair as well as each individual token with
        // the given quote asset
        let mut trigger_update = msg;
        // we want to make sure no updates occur to the state of the virtual pool when
        // we load it
        trigger_update.logs = vec![];

        // add first pair
        let pair0 = Pair(pair.0, self.quote_asset);
        trigger_update.action = make_fake_swap(pair0);
        self.queue_loading(pair0, trigger_update.clone());

        // add second direction
        let pair1 = Pair(pair.1, self.quote_asset);
        trigger_update.action = make_fake_swap(pair1);
        self.queue_loading(pair1, trigger_update);
    }

    /// For a given block number and tx idx, finds the path to the following
    /// tokens and inserts the data into dex_quotes.
    fn update_dex_quotes(&mut self, block: u64, tx_idx: u64, pool_pair: Pair) {
        if pool_pair.0 == pool_pair.1 {
            return
        }

        // query graph for all keys needed to properly query price for a given pair
        let pool_keys = self
            .pair_graph
            .get_path(pool_pair)
            .map(|pairs| {
                PoolKeysForPair(
                    pairs
                        .into_iter()
                        .filter_map(|pair_details| {
                            Some(PoolKeyWithDirection::new(
                                *self.last_update.get(&pair_details.info.pool_addr)?,
                                pair_details.get_base_token(),
                            ))
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();

        if pool_keys.is_empty() {
            debug!(?pool_pair, "no keys found for pair");
            return
        }

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
                    tx.insert(pool_pair, pool_keys);
                } else {
                    let mut tx_pairs = HashMap::default();
                    tx_pairs.insert(pool_pair, pool_keys);
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
                map.insert(pool_pair, pool_keys);

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

        self.update_dex_quotes(block, tx_idx, pool_pair);
        self.update_dex_quotes(block, tx_idx, pool_pair.flip());
        self.update_dex_quotes(block, tx_idx, pair0);
        self.update_dex_quotes(block, tx_idx, pair1);
    }

    fn update_known_state(&mut self, addr: Address, msg: PoolUpdate) {
        let tx_idx = msg.tx_idx;
        let block = msg.block;
        let Some(pool_pair) = msg.get_pair(self.quote_asset) else {
            error!(?addr, "failed to get pair for pool");
            return;
        };

        // fetch the new key and the state applying the transition
        if let Some((key, state)) = self.mut_state.get_mut(&addr).map(|inner| {
            // if we have the pair loaded. increment_state
            let (nonce, snapshot) = inner.increment_state(msg);
            let key = PoolKey {
                pool:         addr,
                run:          self.run,
                batch:        self.batch_id,
                update_nonce: nonce,
            };
            (key, snapshot)
        }) {
            // insert new state snapshot with new key
            self.finalized_state.insert(key, state);
            // update address to new key
            self.last_update.insert(addr, key);

            // generate all variants of the price that might be used in the inspectors
            let pair0 = Pair(pool_pair.0, self.quote_asset);
            let pair1 = Pair(pool_pair.1, self.quote_asset);

            self.update_dex_quotes(block, tx_idx, pool_pair);
            self.update_dex_quotes(block, tx_idx, pool_pair.flip());
            self.update_dex_quotes(block, tx_idx, pair0);
            self.update_dex_quotes(block, tx_idx, pair1);
        } else {
            error!(?addr, "missing pool state for");
        }
    }

    fn can_progress(&self) -> bool {
        self.lazy_loader.requests_for_block(&self.completed_block) == 0
            && self.completed_block < self.current_block
    }

    // called when we try to progress to the next block
    fn try_resolve_block(&mut self) -> Option<(u64, DexPrices)> {
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

        let state = self.finalized_state.clone().into();
        self.completed_block += 1;

        // add new nodes to pair graph
        Some((block, DexPrices::new(state, res)))
    }

    fn on_pool_resolve(&mut self, state: LazyResult) {
        let LazyResult { block, state, load_result } = state;

        if let Some(state) = state {
            let nonce = state.nonce();
            let snap = state.into_snapshot();
            let addr = state.address();

            let key = PoolKey {
                pool:         addr,
                run:          self.run,
                batch:        self.batch_id,
                update_nonce: nonce,
            };

            // init caches
            self.finalized_state.insert(key, snap);
            self.last_update.insert(addr, key);
            self.mut_state.insert(addr, state);

            // pool was initted this block. lets set the override to avoid invalid state
            // if
            if !load_result.is_ok() {
                self.buffer.overrides.entry(block).or_default().insert(addr);
            }
        }
    }

    fn on_close(&mut self) -> Option<(u64, DexPrices)> {
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

        let state = self.finalized_state.clone().into();
        self.completed_block += 1;

        Some((block, DexPrices::new(state, res)))
    }
}

impl<T: TracingProvider> Stream for BrontesBatchPricer<T> {
    type Item = (u64, DexPrices);

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        // because of how heavy this loop is when running, we want to give back to the
        // runtime scheduler less in order to boost performance
        let mut work = 1024;
        loop {
            if let Poll::Ready(s) = self.update_rx.poll_recv(cx).map(|inner| {
                inner.map(|action| match action {
                    DexPriceMsg::Update(update) => {
                        self.on_message(update);
                        true
                    }
                    DexPriceMsg::Closed => false,
                })
            }) {
                if s.is_none() && self.lazy_loader.is_empty() {
                    return Poll::Ready(self.on_close())
                }

                // check to close
                if (self.lazy_loader.is_empty() && self.new_graph_pairs.is_empty())
                    && s.is_some_and(|s| !s)
                {
                    return Poll::Ready(self.on_close())
                }
            }

            // drain all loaded pools
            while let Poll::Ready(Some(state)) = self.lazy_loader.poll_next_unpin(cx) {
                self.on_pool_resolve(state)
            }

            // check if we can progress to the next block.
            let block_prices = self.try_resolve_block();
            if block_prices.is_some() {
                return Poll::Ready(block_prices)
            }

            work -= 1;
            if work == 0 {
                cx.waker().wake_by_ref();
                return Poll::Pending
            }
        }
    }
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

#[cfg(test)]
pub mod test {
    use std::env;

    use brontes_classifier::*;
    use brontes_core::{decoding::parser::TraceParser, init_trace_parser, init_tracing};
    use brontes_database_libmdbx::{
        tables::{AddressToProtocol, AddressToTokens},
        Libmdbx,
    };
    use reth_db::{cursor::DbCursorRO, transaction::DbTx};
    use tokio::sync::mpsc::unbounded_channel;

    use super::*;

    async fn init(
        libmdbx: &Libmdbx,
        rx: UnboundedReceiver<PoolUpdate>,
        quote: Address,
        block: u64,
        parser: &TraceParser<'_, Box<dyn TracingProvider>>,
    ) -> BrontesBatchPricer<Box<dyn TracingProvider>> {
        let tx = libmdbx.ro_tx().unwrap();
        let binding_tx = libmdbx.ro_tx().unwrap();
        let mut all_addr_to_tokens = tx.cursor_read::<AddressToTokens>().unwrap();
        let mut pairs = HashMap::new();

        let pairs = libmdbx.addresses_inited_before(start_block).unwrap();

        let mut rest_pairs = HashMap::default();
        for i in start_block + 1..=end_block {
            let pairs = libmdbx.addresses_init_block(i).unwrap();
            rest_pairs.insert(i, pairs);
        }

        info!("initing pair graph");
        let pair_graph = PairGraph::init_from_hashmap(pairs);

        BrontesBatchPricer::new(quote, 0, 0, pair_graph, rx, parser.get_tracer(), block, rest_pairs)
    }
    #[tokio::test]
    async fn test_pool() {
        dotenv::dotenv().ok();
        init_tracing();
        info!("initing tests");

        let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
        let libmdbx = Libmdbx::init_db(brontes_db_endpoint, None).unwrap();
        let (tx, rx) = unbounded_channel();

        let (a, b) = unbounded_channel();
        let tracer =
            brontes_core::init_trace_parser(tokio::runtime::Handle::current(), a, &libmdbx, 10);
        let quote = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
            .parse()
            .unwrap();

        let mut pricer = init(&libmdbx, rx, quote, 18500000, &tracer).await;

        info!("starting tests");

        let handle = tokio::spawn(async move {
            let res = pricer.next().await;
            (pricer, res)
        });
        // weth
        let t0: Address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
            .parse()
            .unwrap();

        // usdt
        let t1: Address = "0xdAC17F958D2ee523a2206206994597C13D831ec7"
            .parse()
            .unwrap();

        // shib
        let t3: Address = "0x3e34eabf5858a126cb583107e643080cee20ca64"
            .parse()
            .unwrap();

        // lets send pools we want to be able to swap through
        let mut poolupdate = PoolUpdate {
            block:  18500000,
            tx_idx: 0,
            logs:   vec![],
            action: BrontesBatchPricer::<Box<dyn TracingProvider>>::make_fake_swap(t0, t1),
        };

        // send these two txes for the given block
        let _ = tx.send(poolupdate.clone()).unwrap();
        poolupdate.tx_idx = 69;
        poolupdate.action = BrontesBatchPricer::<Box<dyn TracingProvider>>::make_fake_swap(t0, t3);
        let _ = tx.send(poolupdate.clone()).unwrap();

        info!("triggering next block");
        // trigger next block
        poolupdate.block += 1;
        poolupdate.action = BrontesBatchPricer::<Box<dyn TracingProvider>>::make_fake_swap(t1, t3);
        let _ = tx.send(poolupdate.clone()).unwrap();

        let (handle, dex_prices) = handle.await.unwrap();

        let (block, prices) = dex_prices.unwrap();
        info!(?prices, "got prices");

        // default pairs
        let p0 = Pair(t0, t1);
        let p1 = p0.clone().flip();

        let p2 = Pair(t0, t3);
        let p3 = p2.clone().flip();

        // pairs with quote
        let p4 = Pair(t0, quote);
        let p5 = Pair(t1, quote);
        let p6 = Pair(t3, quote);

        // we should have p0 and p1 at index 0 in the vector
        assert!(prices.price_after(p0, 0).is_some());
        assert!(prices.price_after(p1, 0).is_some());
        assert!(prices.price_after(p4, 0).is_some());
        assert!(prices.price_after(p5, 0).is_some());

        // we should have t0, t3 and t1, t3 at 69
        assert!(prices.price_after(p2, 69).is_some());
        assert!(prices.price_after(p3, 69).is_some());
        assert!(prices.price_after(p4, 69).is_some());
        assert!(prices.price_after(p6, 69).is_some());
    }

    // TODO: test on:
    // 0xc04c9540da17ee0e1043e3e07087e1f1149c788e5fe70773f64866f81269c6e6
    // TODO: 0x58cb209340e36a688ad75bc1166b6ad9f427840a8206a015e45bca6a41cb30b1
}
