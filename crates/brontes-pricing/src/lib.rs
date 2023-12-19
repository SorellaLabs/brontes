#![allow(unused)]
pub mod exchanges;
mod graph;
pub mod types;
use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    sync::Arc,
    task::Poll,
};

use alloy_primitives::Address;
use brontes_types::{extra_processing::Pair, traits::TracingProvider};
use exchanges::lazy::LazyExchangeLoader;
pub use exchanges::*;
use futures::{Future, Stream, StreamExt};
pub use graph::PairGraph;
use serde::de;
use tokio::sync::mpsc::Receiver;
use tracing::info;
use types::{DexPrices, DexQuotes, PoolKeyWithDirection, PoolStateSnapShot, PoolUpdate};

use crate::types::{PoolKey, PoolKeysForPair, PoolState};

pub struct BrontesBatchPricer<T: TracingProvider> {
    quote_asset: Address,
    run:         u64,
    batch_id:    u64,

    update_rx: Receiver<PoolUpdate>,

    current_block:   u64,
    completed_block: u64,

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
        update_rx: Receiver<PoolUpdate>,
        provider: Arc<T>,
        current_block: u64,
    ) -> Self {
        Self {
            quote_asset,
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
            completed_block: current_block - 1,
        }
    }

    fn on_message(&mut self, msg: PoolUpdate) {
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
    }

    fn on_new_pool(&mut self, msg: PoolUpdate) {
        let Some(pair) = msg.get_pair() else { return };
        info!(?msg, "on new pool");

        // we add support for fetching the pair as well as each individual token with
        // the given quote asset
        let mut new_pair_set = self
            .pair_graph
            .get_path(Pair(pair.0, self.quote_asset))
            .collect::<HashSet<_>>();

        new_pair_set.extend(
            self.pair_graph
                .get_path(Pair(pair.1, self.quote_asset))
                .collect::<HashSet<_>>(),
        );

        new_pair_set.extend(
            self.pair_graph
                .get_path(Pair(pair.0, pair.1))
                .collect::<HashSet<_>>(),
        );

        for info in new_pair_set.into_iter().flatten() {
            self.lazy_loader.lazy_load_exchange(
                info.info.pool_addr,
                msg.block - 1,
                info.info.dex_type,
            )
        }
    }

    fn update_known_state(&mut self, addr: Address, msg: PoolUpdate) {
        info!(?addr, "update known state");
        let tx_idx = msg.tx_idx;
        let block = msg.block;
        let pool_pair = msg
            .get_pair()
            .expect("got a non exchange state related update");

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

            // fetch all pool keys for a given pair
            let pool_keys = self
                .pair_graph
                .get_path(pool_pair)
                .map(|pairs| {
                    PoolKeysForPair(
                        pairs
                            .into_iter()
                            .map(|pair_details| {
                                PoolKeyWithDirection::new(
                                    *self.last_update.get(&pair_details.info.pool_addr).unwrap(),
                                    pair_details.get_base_token(),
                                )
                            })
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>();

            match self.dex_quotes.entry(block) {
                Entry::Occupied(mut quotes) => {
                    let q = quotes.get_mut();
                    let size = q.0.len();

                    // make sure to pad the vector to the proper index
                    for _ in size..=tx_idx as usize {
                        q.0.push(None)
                    }

                    // insert the new keys
                    let mut tx_pairs = q.0.remove(tx_idx as usize).unwrap_or_default();
                    tx_pairs.insert(pool_pair, pool_keys);
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

                    vec.push(Some(map));
                    v.insert(DexQuotes(vec));
                }
            }
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
            self.on_message(update);
        }
        // if there are no requests and we have moved onto processing the next block,
        // then we will resolve this block. otherwise we will wait
        if self.lazy_loader.requests_for_block(&self.completed_block) == 0
            && self.completed_block < self.current_block
        {
            let block = self.completed_block;

            let res = self
                .dex_quotes
                .remove(&self.completed_block)
                .unwrap_or(DexQuotes(vec![]));

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
        while let Poll::Ready(Some((state, updates))) = self.lazy_loader.poll_next_unpin(cx) {
            if let Some(update) = self.on_pool_resolve(state, updates) {
                return Poll::Ready(Some(update))
            }
        }

        while let Poll::Ready(s) = self
            .update_rx
            .poll_recv(cx)
            .map(|inner| inner.map(|update| self.on_message(update)))
        {
            if s.is_none() && self.lazy_loader.is_empty() {
                return Poll::Ready(None)
            }
        }

        cx.waker().wake_by_ref();
        Poll::Pending
    }
}
