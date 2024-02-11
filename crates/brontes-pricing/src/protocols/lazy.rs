use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    pin::Pin,
    sync::Arc,
    task::Poll,
};

use alloy_primitives::Address;
use brontes_types::{pair::Pair, traits::TracingProvider, unzip_either::IterExt};
use futures::{stream::FuturesOrdered, Future, Stream, StreamExt};
use itertools::Itertools;
use tracing::error;

use crate::{errors::AmmError, protocols::LoadState, types::PoolState, Protocol};

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
        protocol:     Protocol,
        pool_address: Address,
        pool_pair:    Pair,
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
    pool_load_futures: MultiBlockPoolFutures,
    /// addresses currently being processed. to the blocks of the address we are
    /// fetching state for
    pool_buf: HashMap<Address, Vec<u64>>,
    /// requests we are processing for a given block.
    req_per_block: HashMap<u64, u64>,
    /// all current parent pairs with all the state that is required for there
    /// subgraph to be loaded
    parent_pair_state_loading: HashMap<Pair, (u64, Option<u64>, HashSet<Address>)>,
    /// All current request addresses to subgraph pair that requested the
    /// loading. in the case that a pool fails to load, we need all subgraph
    /// pairs that are dependent on the node in order to remove it from the
    /// subgraph and possibly reconstruct it.
    protocol_address_to_parent_pairs: HashMap<Address, Vec<(u64, Pair)>>,
}

impl<T: TracingProvider> LazyExchangeLoader<T> {
    pub fn new(provider: Arc<T>) -> Self {
        Self {
            pool_buf: HashMap::default(),
            pool_load_futures: MultiBlockPoolFutures::new(),
            provider,
            req_per_block: HashMap::default(),
            protocol_address_to_parent_pairs: HashMap::default(),
            parent_pair_state_loading: HashMap::default(),
        }
    }

    pub fn is_loading(&self, k: &Address) -> bool {
        self.pool_buf.contains_key(k)
    }

    pub fn is_empty(&self) -> bool {
        self.pool_load_futures.is_empty()
    }

    pub fn can_progress(&self, block: &u64) -> bool {
        self.req_per_block.get(block).copied().unwrap_or(0) == 0
    }

    pub fn is_loading_block(&self, k: &Address) -> Option<Vec<u64>> {
        self.pool_buf.get(k).cloned()
    }

    pub fn pairs_to_verify(&mut self) -> Vec<(u64, Option<u64>, Pair)> {
        let mut res = Vec::new();
        self.parent_pair_state_loading
            .retain(|pair, (block, id, deps)| {
                if deps.is_empty() {
                    res.push((*block, *id, *pair));
                    return false
                }
                true
            });

        res
    }

    pub fn add_state_trackers(
        &mut self,
        block: u64,
        id: Option<u64>,
        address: Address,
        parent_pair: Pair,
    ) {
        *self.req_per_block.entry(block).or_default() += 1;
        self.pool_buf.entry(address).or_default().push(block);

        self.add_protocol_parent(block, id, address, parent_pair);
    }

    pub fn add_protocol_parent(
        &mut self,
        block: u64,
        id: Option<u64>,
        address: Address,
        parent_pair: Pair,
    ) {
        self.protocol_address_to_parent_pairs
            .entry(address)
            .or_default()
            .push((block, parent_pair));

        match self.parent_pair_state_loading.entry(parent_pair) {
            Entry::Vacant(v) => {
                let mut set = HashSet::new();
                set.insert(address);
                v.insert((block, id, set));
            }
            Entry::Occupied(mut o) => {
                let (cur_block, _id, entry) = o.get_mut();
                if *cur_block != block {
                    tracing::error!(
                        ?address,
                        ?parent_pair,
                        "cur block != block when adding parent"
                    );
                }
                *cur_block = block;
                entry.insert(address);
            }
        }
    }

    // removes state trackers return a list of pairs that is dependent on the state
    pub fn remove_state_trackers(&mut self, block: u64, address: &Address) -> Vec<Pair> {
        if let Entry::Occupied(mut o) = self.pool_buf.entry(*address) {
            let vec = o.get_mut();
            vec.retain(|b| *b != block);

            if vec.is_empty() {
                o.remove_entry();
            }
        }

        if let Entry::Occupied(mut o) = self.req_per_block.entry(block) {
            *(o.get_mut()) -= 1;
        }

        // only remove for state loading for the given block
        let removed =
            if let Entry::Occupied(mut o) = self.protocol_address_to_parent_pairs.entry(*address) {
                let entry = o.get_mut();
                let mut finished_pairs = Vec::new();
                entry.retain(|(target_block, pair)| {
                    if *target_block == block {
                        finished_pairs.push(*pair);
                        return false
                    }
                    true
                });
                if entry.is_empty() {
                    o.remove_entry();
                }

                finished_pairs
            } else {
                vec![]
            };

        removed.iter().for_each(|pair| {
            if let Entry::Occupied(mut o) = self.parent_pair_state_loading.entry(*pair) {
                let (_, _, entry) = o.get_mut();
                entry.remove(address);
            }
        });

        removed
    }

    pub fn lazy_load_exchange(
        &mut self,
        parent_pair: Pair,
        pool_pair: Pair,
        id: Option<u64>,
        address: Address,
        block_number: u64,
        ex_type: Protocol,
    ) {
        let provider = self.provider.clone();
        self.add_state_trackers(block_number, id, address, parent_pair);

        let fut = ex_type.try_load_state(address, provider, block_number, pool_pair);
        self.pool_load_futures
            .add_future(block_number, Box::pin(fut));
    }
}

impl<T: TracingProvider> Stream for LazyExchangeLoader<T> {
    type Item = LazyResult;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        if let Poll::Ready(Some(result)) = self.pool_load_futures.poll_next_unpin(cx) {
            match result {
                Ok((block, addr, state, load)) => {
                    self.remove_state_trackers(block, &addr);

                    let res = LazyResult { block, state: Some(state), load_result: load };
                    Poll::Ready(Some(res))
                }
                Err((pool_address, dex, block, pool_pair, err)) => {
                    error!(%err, ?pool_address,"lazy load failed");

                    let _dependent_pairs = self.remove_state_trackers(block, &pool_address);

                    let res = LazyResult {
                        state: None,
                        block,
                        load_result: LoadResult::Err { pool_pair, pool_address, protocol: dex },
                    };
                    Poll::Ready(Some(res))
                }
            }
        } else {
            Poll::Pending
        }
    }
}

/// The MultiBlockPoolFutures struct is a collection of FuturesOrdered in which,
/// pool futures which are from earlier blocks are loaded first. This allows us
/// to load state and verify pairs for blocks ahead while we wait for the
/// current block pairs to all be verified making the pricing module very
/// efficient.
pub struct MultiBlockPoolFutures(
    HashMap<u64, FuturesOrdered<BoxedFuture<Result<PoolFetchSuccess, PoolFetchError>>>>,
);
impl Default for MultiBlockPoolFutures {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiBlockPoolFutures {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn add_future(
        &mut self,
        block: u64,
        fut: BoxedFuture<Result<PoolFetchSuccess, PoolFetchError>>,
    ) {
        self.0.entry(block).or_default().push_back(fut);
    }
}

impl Stream for MultiBlockPoolFutures {
    type Item = Result<PoolFetchSuccess, PoolFetchError>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        if self.0.is_empty() {
            return Poll::Ready(None)
        }

        let (mut result, empty): (Vec<_>, Vec<_>) = self
            .0
            .iter_mut()
            .sorted_by(|(b0, _), (b1, _)| b0.cmp(b1))
            .map(|(block, futures)| {
                let res = if let Poll::Ready(result) = futures.poll_next_unpin(cx) {
                    result
                } else {
                    None
                };

                if futures.is_empty() {
                    return (res, Some(*block))
                }

                (res, None)
            })
            .take_while_inclusive(|(res, _)| res.is_none())
            .unzip_either();

        empty.into_iter().for_each(|cleared| {
            let _ = self.0.remove(&cleared);
        });

        if let Some(result) = result.pop() {
            return Poll::Ready(Some(result))
        }

        Poll::Pending
    }
}
