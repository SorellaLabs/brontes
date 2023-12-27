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
use types::{DexPrices, DexQuotes, PoolKeyWithDirection, PoolStateSnapShot, PoolUpdate};

use crate::types::{PoolKey, PoolKeysForPair, PoolState};

pub struct StateBuffer {
    pub updates:   HashMap<u64, VecDeque<(Address, PoolUpdate)>>,
    pub overrides: HashMap<u64, HashSet<Address>>,
}
impl StateBuffer {
    pub fn new() -> Self {
        Self { updates: HashMap::default(), overrides: HashMap::default() }
    }
}

pub struct BrontesBatchPricer<T: TracingProvider> {
    quote_asset: Address,
    run:         u64,
    batch_id:    u64,

    update_rx: UnboundedReceiver<PoolUpdate>,

    current_block:   u64,
    completed_block: u64,

    buffer: StateBuffer,

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
        update_rx: UnboundedReceiver<PoolUpdate>,
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

    // will always be in order
    fn on_message(&mut self, msg: PoolUpdate) {
        // we want to capture these
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

        if msg.action.is_transfer() {
            let pair = msg.get_pair(self.quote_asset);

            self.update_dex_quotes(msg.block, msg.tx_idx, msg.get_pair(self.quote_asset).unwrap());
            self.update_dex_quotes(
                msg.block,
                msg.tx_idx,
                msg.get_pair(self.quote_asset).unwrap().flip(),
            );
            return
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
            self.on_new_pool(msg)
        }
    }

    fn queue_loading(&mut self, pair: Pair, trigger_update: PoolUpdate) {
        for pool_info in self
            .pair_graph
            .get_path(pair)
            .flatten()
            .filter(|f| {
                !(self.mut_state.contains_key(&f.info.pool_addr)
                    || self.lazy_loader.is_loading(&f.info.pool_addr))
            })
            .collect::<Vec<_>>()
        {
            // load exchange
            self.lazy_loader.lazy_load_exchange(
                pool_info.info.pool_addr,
                trigger_update.block,
                pool_info.info.dex_type,
            );

            // we buffer the update for all of the pool state with there specific addresses
            // this is so that when the pool resolves,
            self.buffer
                .updates
                .entry(trigger_update.block)
                .or_default()
                .push_back((pool_info.info.pool_addr, trigger_update.clone()));
        }
    }

    fn on_new_pool(&mut self, msg: PoolUpdate) {
        let Some(pair) = msg.get_pair(self.quote_asset) else {
            warn!(pool_update=?msg, "was not able to derive pair from update");
            return
        };

        // we add support for fetching the pair as well as each individual token with
        // the given quote asset
        let mut trigger_update = msg.clone();
        trigger_update.logs = vec![];

        // add first direction
        let pair0 = Pair(pair.0, self.quote_asset);
        trigger_update.action = make_fake_swap(pair0);
        self.queue_loading(pair0, trigger_update.clone());

        // add second direction
        let pair1 = Pair(pair.1, self.quote_asset);
        trigger_update.action = make_fake_swap(pair1);
        self.queue_loading(pair1, trigger_update);

        // add default pair
        self.queue_loading(pair, msg.clone());
        // flipped default
        self.queue_loading(pair.flip(), msg.clone());
    }

    fn update_dex_quotes(&mut self, block: u64, tx_idx: u64, pool_pair: Pair) {
        if pool_pair.0 == pool_pair.1 {
            return
        }

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

        match self.dex_quotes.entry(block) {
            Entry::Occupied(mut quotes) => {
                let q = quotes.get_mut();
                let size = q.0.len();
                // make sure to pad the vector to the proper index
                for _ in size..=tx_idx as usize {
                    q.0.push(None)
                }
                // take the empty if exists
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
                for _ in 0..tx_idx as usize {
                    vec.push(None);
                }
                // insert
                let mut map = HashMap::new();
                map.insert(pool_pair, pool_keys);

                vec.push(Some(map));
                v.insert(DexQuotes(vec));
            }
        }
    }

    // doesn't apply the state transfer given the pool is from end of block
    fn init_new_pool_override(&mut self, addr: Address, msg: PoolUpdate) {
        let tx_idx = msg.tx_idx;
        let block = msg.block;
        let Some(pool_pair) = msg.get_pair(self.quote_asset) else { return };

        if let Some((key, state)) = self.mut_state.get_mut(&addr).map(|inner| {
            let nonce = inner.nonce();
            let snap = inner.into_snapshot();

            let key = PoolKey {
                pool:         addr,
                run:          self.run,
                batch:        self.batch_id,
                update_nonce: nonce,
            };
            (key, snap)
        }) {
            // insert new state snapshot with new key
            self.finalized_state.insert(key, state);
            // update address to new key
            self.last_update.insert(addr, key);

            let pair0 = Pair(pool_pair.0, self.quote_asset);
            let pair1 = Pair(pool_pair.1, self.quote_asset);

            self.update_dex_quotes(block, tx_idx, pool_pair);
            self.update_dex_quotes(block, tx_idx, pool_pair.flip());
            self.update_dex_quotes(block, tx_idx, pair0);
            self.update_dex_quotes(block, tx_idx, pair1);
        }
    }

    fn update_known_state(&mut self, addr: Address, msg: PoolUpdate) {
        let tx_idx = msg.tx_idx;
        let block = msg.block;
        let Some(pool_pair) = msg.get_pair(self.quote_asset) else { return };

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

            let pair0 = Pair(pool_pair.0, self.quote_asset);
            let pair1 = Pair(pool_pair.1, self.quote_asset);

            self.update_dex_quotes(block, tx_idx, pool_pair);
            self.update_dex_quotes(block, tx_idx, pool_pair.flip());
            self.update_dex_quotes(block, tx_idx, pair0);
            self.update_dex_quotes(block, tx_idx, pair1);
        }
    }

    fn can_progress(&self) -> bool {
        self.lazy_loader.requests_for_block(&self.completed_block) == 0
            && self.completed_block < self.current_block
    }

    fn try_resolve_block(&mut self) -> Option<(u64, DexPrices)> {
        // if there are still requests for the given block or the current block isn't
        // complete yet, then we wait
        if !self.can_progress() {
            return None
        }
        // if all block requests are complete, lets apply all the state transitions we
        // had for the given block which will allow us to generate all pricing
        if let (Some(buffer), Some(overrides)) = (
            self.buffer.updates.remove(&self.completed_block),
            self.buffer.overrides.remove(&self.completed_block),
        ) {
            for (address, update) in buffer {
                if overrides.contains(&address) {
                    self.init_new_pool_override(address, update)
                } else {
                    self.update_known_state(address, update);
                }
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

            if !load_result.is_ok() {
                self.buffer.overrides.entry(block).or_default().insert(addr);
            }
        } else {
            error!("failed to load a pool");
        }
    }

    fn on_close(&mut self) -> Option<(u64, DexPrices)> {
        info!(?self.completed_block,"getting ready to calc dex prices");
        // if all block requests are complete, lets apply all the state transitions we
        // had for the given block which will allow us to generate all pricing
        if let (Some(buffer), Some(overrides)) = (
            self.buffer.updates.remove(&self.completed_block),
            self.buffer.overrides.remove(&self.completed_block),
        ) {
            for (address, update) in buffer {
                if overrides.contains(&address) {
                    self.init_new_pool_override(address, update)
                } else {
                    self.update_known_state(address, update);
                }
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
            if let Poll::Ready(s) = self
                .update_rx
                .poll_recv(cx)
                .map(|inner| inner.map(|update| self.on_message(update)))
            {
                if s.is_none() && self.lazy_loader.is_empty() {
                    return Poll::Ready(self.on_close())
                }

                if self.lazy_loader.is_empty() && self.new_graph_pairs.is_empty() {
                    info!(batch=self.batch_id,block=?self.current_block, "brontes pricing completed");

                    return Poll::Ready(self.on_close())
                }
            }

            // drain all loaded pools
            while let Poll::Ready(Some(state)) = self.lazy_loader.poll_next_unpin(cx) {
                self.on_pool_resolve(state)
            }

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

const fn make_fake_swap(pair: Pair) -> Actions {
    Actions::Swap(NormalizedSwap {
        index:      0,
        from:       Address::ZERO,
        recipient:  Address::ZERO,
        pool:       Address::ZERO,
        token_in:   pair.0,
        token_out:  pair.1,
        amount_in:  U256::ZERO,
        amount_out: U256::ZERO,
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
