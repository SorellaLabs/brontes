pub mod exchanges;
mod graph;
pub mod types;
use std::collections::{hash_map::Entry, HashMap};

use alloy_primitives::Address;
use brontes_types::extra_processing::Pair;
use exchanges::lazy::LazyExchangeLoader;
use futures::Future;
use graph::PairGraph;
use tokio::sync::mpsc::Receiver;
use types::{DexPrices, DexQuotes, PoolStateSnapShot, PoolUpdate};

use crate::types::{PoolKey, PoolState};

pub struct BrontesBatchPricer {
    quote_asset: Address,
    run:         u64,
    batch_id:    u64,

    pair_graph:  PairGraph,
    // TODO: this will be db type;
    pool_2_pair: HashMap<Address, Pair>,

    update_rx:   Receiver<PoolUpdate>,
    lazy_loader: LazyExchangeLoader,
    // we use this to queue up the updates that we can apply on finalization
    mut_state:   HashMap<Address, PoolState>,

    // tracks the last updated key for the given pool
    last_update: HashMap<Address, PoolKey>,

    pairs:           HashMap<u64, DexQuotes>,
    finalized_state: HashMap<PoolKey, PoolStateSnapShot>,
}

impl BrontesBatchPricer {
    pub fn new(
        run: u64,
        batch_id: u64,
        quote_asset: Address,
        loader: LazyExchangeLoader,
        update_rx: Receiver<PoolUpdate>,
    ) -> Self {
        todo!()
    }

    fn on_message(&mut self, msg: PoolUpdate) {
        let addr = msg.get_pool_address();
        if let Some((key, state)) = self.mut_state.get_mut(&addr).map(|inner| {
            let (nonce, snapshot) = inner.increment_state(msg);
            let key = PoolKey {
                pool:         addr,
                run:          self.run,
                batch:        self.batch_id,
                update_nonce: nonce,
            };
            (key, snapshot)
        }) {
            let pool_pair = *self.pool_2_pair.get(&addr).unwrap();
            match self.pairs.entry(msg.block) {
                Entry::Occupied(mut quotes) => {
                    let q = quotes.get_mut();
                    let size = q.0.len();

                    for _ in size..=msg.tx_idx {
                        q.0.push(None)
                    }

                    let mut tx_pairs = q.0.remove(msg.tx_idx).unwrap_or_default();
                    tx_pairs.insert(pool_pair, vec![key]);
                }
                Entry::Vacant(v) => {
                    let mut vec = Vec::new();
                    for _ in 0..msg.tx_idx {
                        vec.push(None);
                    }
                    let mut map = HashMap::new();
                    map.insert(pool_pair, vec![key]);

                    vec.push(Some(map));
                    v.insert(DexQuotes(vec));

                    self.finalized_state.insert(key, state);
                }
            }

            return
        }

        if self.lazy_loader.is_loading(&addr) {
            self.lazy_loader.buffer_update(&addr, msg);
            return
        }

        self.lazy_loader.lazy_load_exchange(addr, msg.block - 1, ())
    }
}

impl Future for BrontesBatchPricer {
    type Output = HashMap<u64, DexPrices>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        todo!()
    }
}
