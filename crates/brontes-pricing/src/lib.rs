pub mod exchanges;
pub mod types;

use std::collections::{BTreeMap, HashMap};

use alloy_primitives::Address;
use futures::Future;
use tokio::sync::mpsc::Receiver;
use types::{DexPrices, DexQuotes, PairPriceMessage, PoolUpdate};

use crate::types::{PoolKey, PoolState};

pub struct BrontesBatchPricer {
    quote_asset:     Address,
    finalized_state: u64,

    update_rx: Receiver<PairPriceMessage>,

    // we use this to queue up the updates that we can apply on finalization
    update_buffer: BTreeMap<u64, Vec<PoolUpdate>>,

    // tracks the last updated key for the given pool
    last_update: HashMap<Address, PoolKey>,
    pairs:       HashMap<u64, DexQuotes>,
    state:       HashMap<PoolKey, PoolState>,
}

impl BrontesBatchPricer {}

impl Future for BrontesBatchPricer {
    type Output = HashMap<u64, DexPrices>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        todo!()
    }
}
