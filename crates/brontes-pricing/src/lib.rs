pub mod exchanges;
mod graph;
pub mod types;

use std::collections::{BTreeMap, HashMap};

use alloy_primitives::Address;
use exchanges::lazy::LazyExchangeLoader;
use futures::Future;
use graph::PairGraph;
use tokio::sync::mpsc::Receiver;
use types::{DexPrices, DexQuotes, PairPriceMessage, PoolStateSnapShot, PoolUpdate};

use crate::types::{PoolKey, PoolState};

pub struct BrontesBatchPricer {
    quote_asset: Address,
    batch_id:    u64,

    pair_graph: PairGraph,

    update_rx:   Receiver<PairPriceMessage>,
    lazy_loader: LazyExchangeLoader,

    // we use this to queue up the updates that we can apply on finalization
    mut_state: HashMap<Address, PoolState>,

    // tracks the last updated key for the given pool
    last_update:     HashMap<Address, PoolKey>,
    pairs:           HashMap<u64, DexQuotes>,
    finalized_state: HashMap<PoolKey, PoolStateSnapShot>,
}

impl BrontesBatchPricer {
    pub fn new(
        quote_asset: Address,
        batch_id: u64,
        loader: LazyExchangeLoader,
        update_rx: Receiver<PairPriceMessage>,
    ) -> Self {
        todo!()
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
