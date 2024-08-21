use std::{collections::hash_map::Entry, pin::Pin, sync::Arc, task::Poll};

use alloy_primitives::Address;
use brontes_metrics::pricing::DexPricingMetrics;
use brontes_types::{
    pair::Pair, traits::TracingProvider, unzip_either::IterExt, BrontesTaskExecutor, FastHashMap,
    FastHashSet,
};
use futures::{stream::FuturesOrdered, Future, Stream, StreamExt};
use itertools::Itertools;
use tokio::task::JoinError;

use crate::{
    errors::AmmError,
    protocols::LoadState,
    types::{PairWithFirstPoolHop, PoolState},
    Protocol,
};

pub(crate) type PoolFetchError = (Address, Protocol, u64, Pair, PairWithFirstPoolHop, AmmError);
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
        pool_pair:    Pair,
        pair:         PairWithFirstPoolHop,
        pool_address: Address,
        deps:         Vec<PairWithFirstPoolHop>,
        block:        u64,
    },
}
impl LoadResult {
    pub fn is_ok(&self) -> bool {
        matches!(self, LoadResult::Ok)
    }
}

pub struct LazyResult {
    pub state:           Option<PoolState>,
    pub block:           u64,
    pub load_result:     LoadResult,
    pub dependent_count: u64,
}

#[derive(Debug)]
pub struct PairStateLoadingProgress {
    pub block:         u64,
    pub id:            Option<u64>,
    pub pending_pools: FastHashSet<Address>,
}

type BoxedFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;
type BlockNumber = u64;

/// Deals with the lazy loading of new exchange state, and tracks loading of new
/// state for a given block.
pub struct LazyExchangeLoader<T: TracingProvider> {
    provider:          Arc<T>,
    pool_load_futures: MultiBlockPoolFutures,
    /// addresses currently being processed. to the blocks of the address we are
    /// fetching state for
    pool_buf:          FastHashMap<Address, FastHashSet<BlockNumber>>,
    /// requests we are processing for a given block.
    req_per_block:     FastHashMap<BlockNumber, u64>,
    state_tracking:    LoadingStateTracker,
    ex:                BrontesTaskExecutor,
}

impl<T: TracingProvider> LazyExchangeLoader<T> {
    pub fn new(provider: Arc<T>, ex: BrontesTaskExecutor) -> Self {
        Self {
            state_tracking: LoadingStateTracker::default(),
            pool_buf: FastHashMap::default(),
            pool_load_futures: MultiBlockPoolFutures::new(),
            provider,
            req_per_block: FastHashMap::default(),
            ex,
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

    pub fn is_loading_block(&self, k: &Address) -> Option<FastHashSet<u64>> {
        self.pool_buf.get(k).cloned()
    }

    /// subgraph to be loaded
    pub fn pairs_to_verify(&mut self) -> Vec<(u64, Option<u64>, PairWithFirstPoolHop)> {
        self.state_tracking.return_pairs_ready_for_loading()
    }

    pub fn full_failure(&mut self, pair: PairWithFirstPoolHop) {
        self.state_tracking.pool_dep_failure(pair);
    }

    pub fn add_state_trackers(
        &mut self,
        block: u64,
        id: Option<u64>,
        address: Address,
        pair: PairWithFirstPoolHop,
    ) {
        *self.req_per_block.entry(block).or_default() += 1;
        self.pool_buf.entry(address).or_default().insert(block);
        self.add_protocol_parent(block, id, address, pair)
    }

    // removes state trackers return a list of pairs that is dependent on the state
    pub fn remove_state_trackers(
        &mut self,
        block: u64,
        address: &Address,
    ) -> Vec<PairWithFirstPoolHop> {
        if let Some(i) = self.pool_buf.get_mut(address) {
            i.remove(&block);
        }

        if let Some(block) = self.req_per_block.get_mut(&block) {
            *block -= 1;
        }

        self.state_tracking.remove_pool(*address, block)
    }

    pub fn add_protocol_parent(
        &mut self,
        block: u64,
        id: Option<u64>,
        address: Address,
        pair: PairWithFirstPoolHop,
    ) {
        self.state_tracking
            .add_protocol_dependent(address, block, pair);
        self.state_tracking
            .add_pending_pool(pair, address, block, id);
    }

    pub fn lazy_load_exchange(
        &mut self,
        pair: PairWithFirstPoolHop,
        pool_pair: Pair,
        id: Option<u64>,
        address: Address,
        block_number: u64,
        ex_type: Protocol,
        metrics: Option<DexPricingMetrics>,
    ) {
        let provider = self.provider.clone();
        self.add_state_trackers(block_number, id, address, pair);

        let fut = ex_type.try_load_state(address, provider, block_number, pool_pair, pair);
        self.pool_load_futures.add_future(
            block_number,
            Box::pin(self.ex.handle().spawn(async move {
                if let Some(metrics) = metrics {
                    metrics.meter_state_load(|| Box::pin(fut)).await
                } else {
                    fut.await
                }
            })),
        );
    }

    pub fn poll_next(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<LazyResult>> {
        if let Poll::Ready(Some(result)) = self.pool_load_futures.poll_next_unpin(cx) {
            match result {
                Ok((block, addr, state, load)) => {
                    let deps = self.remove_state_trackers(block, &addr);

                    let res = LazyResult {
                        block,
                        state: Some(state),
                        load_result: load,
                        dependent_count: deps.len() as u64,
                    };
                    Poll::Ready(Some(res))
                }
                Err((pool_address, dex, block, pool_pair, full_pair, _)) => {
                    let dependent_pairs = self.remove_state_trackers(block, &pool_address);

                    let res = LazyResult {
                        state: None,
                        block,
                        dependent_count: dependent_pairs.len() as u64,
                        load_result: LoadResult::Err {
                            pool_pair,
                            pair: full_pair,
                            pool_address,
                            block,
                            protocol: dex,
                            deps: dependent_pairs,
                        },
                    };
                    Poll::Ready(Some(res))
                }
            }
        } else {
            Poll::Pending
        }
    }
}

type FetchResult = Result<Result<PoolFetchSuccess, PoolFetchError>, JoinError>;

/// The MultiBlockPoolFutures struct is a collection of FuturesOrdered in which,
/// pool futures which are from earlier blocks are loaded first. This allows us
/// to load state and verify pairs for blocks ahead while we wait for the
/// current block pairs to all be verified making the pricing module very
/// efficient.
pub struct MultiBlockPoolFutures(FastHashMap<u64, FuturesOrdered<BoxedFuture<FetchResult>>>);

impl Drop for MultiBlockPoolFutures {
    fn drop(&mut self) {
        let futures_cnt = self.0.values().map(|f| f.len()).sum::<usize>();
        tracing::debug!(target: "brontes::mem", rem_futures=futures_cnt, "current state fetch futures in pricing");
    }
}
impl Default for MultiBlockPoolFutures {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiBlockPoolFutures {
    pub fn new() -> Self {
        Self(FastHashMap::default())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn add_future(
        &mut self,
        block: u64,
        fut: BoxedFuture<Result<Result<PoolFetchSuccess, PoolFetchError>, JoinError>>,
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

        let (mut results, empty): (Vec<_>, Vec<_>) = self
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

        if let Some(result) = results.pop() {
            // no lossless
            assert!(results.is_empty());
            return Poll::Ready(Some(result.unwrap()))
        }

        Poll::Pending
    }
}

#[derive(Debug, Default)]
pub struct LoadingStateTracker {
    pair_loading: FastHashMap<PairWithFirstPoolHop, PairStateLoadingProgress>,
    protocol_address_to_dependent_pairs:
        FastHashMap<Address, Vec<(BlockNumber, PairWithFirstPoolHop)>>,
}

impl LoadingStateTracker {
    pub fn add_protocol_dependent(
        &mut self,
        protocol: Address,
        block_number: u64,
        pair: PairWithFirstPoolHop,
    ) {
        self.protocol_address_to_dependent_pairs
            .entry(protocol)
            .or_default()
            .push((block_number, pair));
    }

    pub fn remove_pool(&mut self, pool: Address, block: u64) -> Vec<PairWithFirstPoolHop> {
        let mut removed = vec![];
        self.protocol_address_to_dependent_pairs.retain(|p, b| {
            if p != &pool {
                return true
            }
            b.retain(|(bn, key)| {
                if &block != bn {
                    return true
                }
                removed.push(*key);
                false
            });

            !b.is_empty()
        });

        removed.iter().for_each(|pair| {
            if let Some(pair_loading) = self.pair_loading.get_mut(pair) {
                pair_loading.pending_pools.remove(&pool);
            }
        });

        removed
    }

    pub fn add_pending_pool(
        &mut self,
        pair: PairWithFirstPoolHop,
        pool: Address,
        block: u64,
        id: Option<u64>,
    ) {
        match self.pair_loading.entry(pair) {
            Entry::Vacant(v) => {
                let mut set = FastHashSet::default();
                set.insert(pool);
                v.insert(PairStateLoadingProgress { block, id, pending_pools: set });
            }
            Entry::Occupied(mut o) => {
                o.get_mut().pending_pools.insert(pool);
            }
        }
    }

    pub fn return_pairs_ready_for_loading(
        &mut self,
    ) -> Vec<(u64, Option<u64>, PairWithFirstPoolHop)> {
        let mut res = Vec::new();
        self.pair_loading.retain(|pair, entries| {
            let PairStateLoadingProgress { block, id, pending_pools } = entries;
            if pending_pools.is_empty() {
                res.push((*block, id.take(), *pair));
                return false
            }
            true
        });

        res
    }

    pub fn pool_dep_failure(&mut self, pair: PairWithFirstPoolHop) {
        let loading = self.pair_loading.remove(&pair);
        self.protocol_address_to_dependent_pairs.retain(|_, v| {
            v.retain(|(_, npair)| npair != &pair);
            !v.is_empty()
        });
    }
}
