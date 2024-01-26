use std::{
    collections::{hash_map::Entry, HashMap, HashSet, VecDeque},
    pin::Pin,
    sync::Arc,
    task::Poll,
};

use alloy_primitives::Address;
use brontes_types::{extra_processing::Pair, traits::TracingProvider};
use futures::{
    future::BoxFuture,
    stream::{FuturesOrdered, FuturesUnordered},
    Future, Stream, StreamExt,
};
use itertools::Itertools;
use tokio::task::JoinHandle;
use tracing::{error, info};

use crate::{
    errors::AmmError, types::PoolState, uniswap_v2::UniswapV2Pool, uniswap_v3::UniswapV3Pool,
    PoolPairInfoDirection, PoolPairInformation, PoolUpdate, Protocol, SubGraphEdge,
};

pub(crate) type PoolFetchError = (Address, Protocol, u64, Pair, AmmError);
pub(crate) type PoolFetchSuccess = (u64, Address, PoolState, LoadResult);

pub enum LoadResult {
    Ok,
    /// because we back query 1 block. if a pool is created at the current
    /// block, this will error. because of this we need to signal this case
    /// to the pricing engine so that we don't apply any state transitions
    /// for this block as it will cause incorrect data
    PoolInitOnBlock,
    Err {
        pool_address: Address,
        pool_pair:    Pair,
        block:        u64,
    },
}
impl LoadResult {
    pub fn is_ok(&self) -> bool {
        matches!(self, LoadResult::Ok)
    }
}

pub struct LazyResult {
    pub state:       Option<PoolState>,
    pub block:       u64,
    pub load_result: LoadResult,
}

type BoxedFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// Deals with the lazy loading of new exchange state, and tracks loading of new
/// state for a given block.
pub struct LazyExchangeLoader<T: TracingProvider> {
    provider: Arc<T>,
    pool_load_futures: FuturesOrdered<BoxedFuture<Result<PoolFetchSuccess, PoolFetchError>>>,
    /// addresses currently being processed.
    pool_buf: HashSet<Address>,
    /// requests we are processing for a given block.
    req_per_block: HashMap<u64, u64>,
    pub parent_pair_state_loading: HashMap<u64, HashMap<Pair, HashSet<Address>>>,
    /// All current request addresses to subgraph pair that requested the
    /// loading. in the case that a pool fails to load, we need all subgraph
    /// pairs that are dependent on the node in order to remove it from the
    /// subgraph and possibly reconstruct it.
    protocol_address_to_parent_pairs: HashMap<Address, Vec<Pair>>,
}

impl<T: TracingProvider> LazyExchangeLoader<T> {
    pub fn new(provider: Arc<T>) -> Self {
        Self {
            pool_buf: HashSet::default(),
            pool_load_futures: FuturesOrdered::default(),
            provider,
            req_per_block: HashMap::default(),
            protocol_address_to_parent_pairs: HashMap::default(),
            parent_pair_state_loading: HashMap::default(),
        }
    }

    pub fn can_progress(&self, block: &u64) -> bool {
        self.req_per_block.get(block).copied().unwrap_or(0) == 0
            && !self.parent_pair_state_loading.contains_key(block)
    }

    pub fn add_protocol_parent(&mut self, block: u64, address: Address, parent_pair: Pair) {
        self.protocol_address_to_parent_pairs
            .entry(address)
            .or_insert(vec![])
            .push(parent_pair);

        self.parent_pair_state_loading
            .entry(block)
            .or_default()
            .entry(parent_pair)
            .or_default()
            .insert(address);
    }

    pub fn get_completed_pairs(&mut self, block: u64) -> Vec<Pair> {
        let mut res = Vec::new();
        self.parent_pair_state_loading.retain(|k, v| {
            if v.values().all(|i| i.is_empty()) {

                res.extend(v.drain().map(|(i, _)| i).collect_vec());
                return false
            }
            true
        });

        res
    }

    pub fn remove_protocol_parents(&mut self, block: u64, address: &Address) -> Vec<Pair> {
        let removed = self
            .protocol_address_to_parent_pairs
            .remove(address)
            .unwrap_or(vec![]);

        removed.iter().for_each(|pair| {
            self.parent_pair_state_loading
                .entry(block)
                .or_default()
                .entry(*pair)
                .or_default()
                .remove(address);
        });

        removed
    }

    pub fn lazy_load_exchange(
        &mut self,
        parent_pair: Pair,
        pool_pair: Pair,
        address: Address,
        block_number: u64,
        ex_type: Protocol,
    ) {
        let provider = self.provider.clone();
        *self.req_per_block.entry(block_number).or_default() += 1;
        self.pool_buf.insert(address);
        self.add_protocol_parent(block_number, address, parent_pair);

        let fut = ex_type.try_load_state(address, provider, block_number, pool_pair);
        self.pool_load_futures.push_back(Box::pin(fut));
    }

    pub fn is_loading(&self, k: &Address) -> bool {
        self.pool_buf.contains(k)
    }

    pub fn is_empty(&self) -> bool {
        self.pool_load_futures.is_empty()
    }
}

impl<T: TracingProvider> Stream for LazyExchangeLoader<T> {
    type Item = LazyResult;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        if let Poll::Ready(Some((result))) = self.pool_load_futures.poll_next_unpin(cx) {
            match result {
                Ok((block, addr, state, load)) => {
                    if let Entry::Occupied(mut o) = self.req_per_block.entry(block) {
                        *(o.get_mut()) -= 1;
                    }

                    self.remove_protocol_parents(block, &addr);

                    self.pool_buf.remove(&addr);
                    let res = LazyResult { block, state: Some(state), load_result: load };
                    Poll::Ready(Some(res))
                }
                Err((pool_address, dex, block, pool_pair, err)) => {
                    error!(%err, ?pool_address,"lazy load failed");
                    if let Entry::Occupied(mut o) = self.req_per_block.entry(block) {
                        *(o.get_mut()) -= 1;
                    }

                    self.pool_buf.remove(&pool_address);
                    let res = LazyResult {
                        state: None,
                        block,
                        load_result: LoadResult::Err { pool_pair, block, pool_address },
                    };
                    Poll::Ready(Some(res))
                }
                _ => Poll::Pending,
            }
        } else {
            Poll::Pending
        }
    }
}
