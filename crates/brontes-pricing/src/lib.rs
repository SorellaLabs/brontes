#![allow(unused)]
pub mod exchanges;
mod graph;
pub mod types;
use std::{
    collections::{hash_map::Entry, HashMap, HashSet, VecDeque},
    sync::Arc,
    task::Poll,
};
use tracing::warn;

use alloy_primitives::{Address, U256};
use brontes_types::{
    extra_processing::Pair,
    normalized_actions::{Actions, NormalizedAction, NormalizedSwap},
    traits::TracingProvider,
};
use ethers::core::k256::elliptic_curve::bigint::Zero;
use exchanges::lazy::LazyExchangeLoader;
pub use exchanges::*;
use futures::{Future, Stream, StreamExt};
pub use graph::PairGraph;
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::info;
use types::{DexPrices, DexQuotes, PoolKeyWithDirection, PoolStateSnapShot, PoolUpdate};

use crate::types::{PoolKey, PoolKeysForPair, PoolState};

pub struct BrontesBatchPricer<T: TracingProvider> {
    quote_asset: Address,
    run:         u64,
    batch_id:    u64,

    update_rx: UnboundedReceiver<PoolUpdate>,

    current_block:   u64,
    completed_block: u64,

    buffer: HashMap<u64, VecDeque<(Address, PoolUpdate)>>,

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
    ) -> Self {
        Self {
            quote_asset,
            buffer: HashMap::default(),
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

    fn on_message(&mut self, msg: PoolUpdate) -> Option<(u64, DexPrices)> {
        if self.lazy_loader.requests_for_block(&self.completed_block) == 0
            && self.completed_block < self.current_block
        {
            info!(?self.completed_block," getting ready to calc dex prices");
            let block = self.completed_block;
            if let Some(buffer) = self.buffer.remove(&self.completed_block) {
                for (address, update) in buffer {
                    self.update_known_state(address, update);
                }
            }

            let res = self
                .dex_quotes
                .remove(&self.completed_block)
                .unwrap_or(DexQuotes(vec![]));
            info!(dex_quotes = res.0.len(), "got dex quotes");

            let state = self.finalized_state.clone().into();
            self.completed_block += 1;

            return Some((self.completed_block, DexPrices::new(state, res)))
        }

        if msg.block > self.current_block {
            self.current_block = msg.block
        }

        let addr = msg.get_pool_address();

        if self.mut_state.contains_key(&addr) {
            self.update_known_state(addr, msg)
        } else if self.lazy_loader.is_loading(&addr) {
            self.lazy_loader.buffer_update(&addr, msg)
        } else {
            self.on_new_pool(msg)
        }

        None
    }

    const fn make_fake_swap(t0: Address, t1: Address) -> Actions {
        Actions::Swap(NormalizedSwap {
            index:      0,
            from:       Address::ZERO,
            pool:       Address::ZERO,
            token_in:   t0,
            token_out:  t1,
            amount_in:  U256::ZERO,
            amount_out: U256::ZERO,
        })
    }

    fn on_new_pool(&mut self, msg: PoolUpdate) {
        let Some(pair) = msg.get_pair() else { return };

        // we add support for fetching the pair as well as each individual token with
        // the given quote asset
        let mut fake_update = msg.clone();
        fake_update.logs = vec![];

        // add first diection
        fake_update.action = Self::make_fake_swap(pair.1, self.quote_asset);
        for info in self
            .pair_graph
            .get_path(Pair(pair.1, self.quote_asset))
            .flatten()
        {
            self.lazy_loader.lazy_load_exchange(
                info.info.pool_addr,
                msg.block - 1,
                info.info.dex_type,
            );

            self.lazy_loader
                .buffer_update(&info.info.pool_addr, fake_update.clone());
        }

        // add second direction
        fake_update.action = Self::make_fake_swap(pair.0, self.quote_asset);
        for info in self
            .pair_graph
            .get_path(Pair(pair.0, self.quote_asset))
            .flatten()
            .collect::<HashSet<_>>()
        {
            self.lazy_loader.lazy_load_exchange(
                info.info.pool_addr,
                msg.block - 1,
                info.info.dex_type,
            );

            self.lazy_loader
                .buffer_update(&info.info.pool_addr, fake_update.clone());
        }

        // add default pair
        for info in self.pair_graph.get_path(Pair(pair.0, pair.1)).flatten() {
            self.lazy_loader.lazy_load_exchange(
                info.info.pool_addr,
                // we want to load state from prev block
                msg.block - 1,
                info.info.dex_type,
            );

            // for the raw pair we always rebuffer
            self.lazy_loader
                .buffer_update(&info.info.pool_addr, msg.clone());
        }
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
                            // TODO: this being a filtermap is wrong because we then can't
                            // garentee all underlying pool weighting. need a bigger refactor
                            // tho so will circle back after i think about it for a bit
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
            warn!(?pool_pair, "no keys found for pair");
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

    fn update_known_state(&mut self, addr: Address, msg: PoolUpdate) {
        let tx_idx = msg.tx_idx;
        let block = msg.block;
        let Some(pool_pair) = msg.get_pair() else { return };

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
            self.update_dex_quotes(block, tx_idx, pair0);
            self.update_dex_quotes(block, tx_idx, pair1);

            // fetch all pool keys for a given pair

            // info!(pair=?pool_pair, %block, ?pool_keys, " adding pricing for
            // key");
        }
    }

    fn on_pool_resolve(
        &mut self,
        state: PoolState,
        updates: Vec<PoolUpdate>,
    ) -> Option<(u64, DexPrices)> {
        info!("on pool resolve");
        let addr = state.address();
        // init state
        self.mut_state.insert(addr, state);

        for update in updates {
            let block = update.block;
            self.buffer
                .entry(block)
                .or_default()
                .push_back((addr, update));
        }

        // if there are no requests and we have moved onto processing the next block,
        // then we will resolve this block. otherwise we will wait
        if self.lazy_loader.requests_for_block(&self.completed_block) == 0
            && self.completed_block < self.current_block
        {
            info!(?self.completed_block,"getting ready to calc dex prices");
            // if all block requests are complete, lets apply all the state transitions we
            // had for the given block which will allow us to generate all pricing
            if let Some(buffer) = self.buffer.remove(&self.completed_block) {
                for (address, update) in buffer {
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

            return Some((block, DexPrices::new(state, res)))
        }

        None
    }
}

impl<T: TracingProvider> Stream for BrontesBatchPricer<T> {
    type Item = (u64, DexPrices);

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let mut work = 1024;
        loop {
            if let Poll::Ready(s) = self
                .update_rx
                .poll_recv(cx)
                .map(|inner| inner.map(|update| self.on_message(update)))
            {
                if let Some(Some(data)) = s {
                    return Poll::Ready(Some(data))
                }

                if s.is_none() && self.lazy_loader.is_empty() {
                    return Poll::Ready(None)
                }
            }

            if let Poll::Ready(Some((state, updates))) = self.lazy_loader.poll_next_unpin(cx) {
                if let Some(update) = self.on_pool_resolve(state, updates) {
                    return Poll::Ready(Some(update))
                }
            }

            work -= 1;
            if work == 0 {
                cx.waker().wake_by_ref();
                return Poll::Pending
            }
        }
    }
}
