pub mod exchanges;
mod graph;
pub mod types;

use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    sync::Arc,
    task::Poll,
};

use alloy_primitives::Address;
use exchanges::lazy::LazyExchangeLoader;
use futures::{Future, StreamExt};
use graph::PairGraph;
use tokio::sync::mpsc::Receiver;
use types::{DexPrices, DexQuotes, PoolStateSnapShot, PoolUpdate};

use crate::types::{PoolKey, PoolState};

pub struct BrontesBatchPricer {
    quote_asset: Address,
    run:         u64,
    batch_id:    u64,

    // all token pairs that we have classified that existed at the given
    // point in time
    pair_graph: PairGraph,

    update_rx:   Receiver<PoolUpdate>,
    /// lazy loads dex pairs so we only fetch init state that is needed
    lazy_loader: LazyExchangeLoader,

    /// mutable version of the pool. used for producing deltas
    mut_state:       HashMap<Address, PoolState>,
    /// tracks the last updated key for the given pool
    last_update:     HashMap<Address, PoolKey>,
    /// quotes
    dex_quotes:      HashMap<u64, DexQuotes>,
    /// the pool-key with finalized state
    finalized_state: HashMap<PoolKey, PoolStateSnapShot>,
}

impl BrontesBatchPricer {
    pub fn new(
        quote_asset: Address,
        run: u64,
        batch_id: u64,
        pair_graph: PairGraph,
        update_rx: Receiver<PoolUpdate>,
    ) -> Self {
        Self {
            quote_asset,
            run,
            batch_id,
            pair_graph,
            finalized_state: HashMap::default(),
            dex_quotes: HashMap::default(),
            lazy_loader: LazyExchangeLoader::new(),
            mut_state: HashMap::default(),
            update_rx,
            last_update: HashMap::default(),
        }
    }

    fn on_message(&mut self, msg: PoolUpdate) {
        let addr = msg.get_pool_address();
        if self.mut_state.contains_key(&addr) {
            self.update_known_state(addr, msg)
        } else if self.lazy_loader.is_loading(&addr) {
            self.lazy_loader.buffer_update(&addr, msg)
        } else {
            self.on_new_pair(msg)
        }
    }

    fn on_new_pair(&mut self, msg: PoolUpdate) {
        let pair = msg
            .get_pair()
            .expect("got a non exchange state related update");

        let new_pair_set = self
            .pair_graph
            .get_path(pair.0, self.quote_asset)
            .chain(self.pair_graph.get_path(pair.1, self.quote_asset))
            .collect::<HashSet<_>>();

        for pool in new_pair_set {
            self.lazy_loader.lazy_load_exchange(pool, msg.block - 1, ())
        }
    }

    fn update_known_state(&mut self, addr: Address, msg: PoolUpdate) {
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
                .get_all_pools(pool_pair)
                .map(|pair_addr| *self.last_update.get(&pair_addr).unwrap())
                .collect::<Vec<_>>();

            match self.dex_quotes.entry(block) {
                Entry::Occupied(mut quotes) => {
                    let q = quotes.get_mut();
                    let size = q.0.len();

                    // make sure to pad the vector to the proper index
                    for _ in size..=tx_idx {
                        q.0.push(None)
                    }

                    // insert the new keys
                    let mut tx_pairs = q.0.remove(tx_idx).unwrap_or_default();
                    tx_pairs.insert(pool_pair, pool_keys);
                }
                Entry::Vacant(v) => {
                    // pad the vec to the tx index
                    let mut vec = Vec::new();
                    for _ in 0..tx_idx {
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

    fn into_results(&mut self) -> HashMap<u64, DexPrices> {
        let dex_quotes = std::mem::take(&mut self.dex_quotes);
        let finalized_state = Arc::new(std::mem::take(&mut self.finalized_state));

        dex_quotes
            .into_iter()
            .map(|(block, quotes)| (block, DexPrices { quotes, state: finalized_state.clone() }))
            .collect()
    }

    fn on_pool_resolve(&mut self, state: PoolState, updates: Vec<PoolUpdate>) {
        let addr = state.address();
        // init state
        self.mut_state.insert(addr, state);
        for update in updates {
            self.on_message(update);
        }
    }
}

impl Future for BrontesBatchPricer {
    type Output = HashMap<u64, DexPrices>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        while let Poll::Ready(Some((state, updates))) = self.lazy_loader.poll_next_unpin(cx) {
            self.on_pool_resolve(state, updates)
        }

        while let Poll::Ready(s) = self
            .update_rx
            .poll_recv(cx)
            .map(|inner| inner.map(|update| self.on_message(update)))
        {
            if s.is_none() && self.lazy_loader.is_empty() {
                return Poll::Ready(self.into_results())
            }
        }

        Poll::Pending
    }
}
