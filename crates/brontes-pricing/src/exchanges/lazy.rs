use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    pin::Pin,
    sync::Arc,
    task::Poll,
};

use alloy_primitives::Address;
use brontes_types::{exchanges::StaticBindingsDb, extra_processing::Pair, traits::TracingProvider};
use futures::{future::BoxFuture, stream::FuturesUnordered, Future, Stream, StreamExt};
use tracing::{error, info};

use crate::{
    errors::AmmError, graph::PoolPairInfoDirection, types::PoolState, uniswap_v2::UniswapV2Pool,
    uniswap_v3::UniswapV3Pool, PoolUpdate,
};

type PoolFetchError = (Address, StaticBindingsDb, u64, AmmError);
type PoolFetchSuccess = (u64, Address, PoolState, LoadResult);

pub enum LoadResult {
    Ok,
    Err,
    // because we back query 1 block. this breaks so we need to instead
    // do a special query
    PoolInitOnBlock,
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

pub struct LazyExchangeLoader<T: TracingProvider> {
    provider:          Arc<T>,
    pool_buf:          HashSet<Address>,
    // this can be unordered as it is in just init state and we don't apply any transitions
    // until all state has been fetched for a given block.
    pool_load_futures:
        FuturesUnordered<BoxFuture<'static, Result<PoolFetchSuccess, PoolFetchError>>>,
    // the different blocks that we are currently fetching
    req_per_block:     HashMap<u64, u64>,
}

impl<T: TracingProvider> LazyExchangeLoader<T> {
    pub fn new(provider: Arc<T>) -> Self {
        Self {
            pool_buf: HashSet::default(),
            pool_load_futures: FuturesUnordered::default(),
            provider,
            req_per_block: HashMap::default(),
        }
    }

    pub fn requests_for_block(&self, block: &u64) -> u64 {
        self.req_per_block.get(block).map(|i| *i).unwrap_or(0)
    }

    pub fn lazy_load_exchange(
        &mut self,
        address: Address,
        block_number: u64,
        ex_type: StaticBindingsDb,
    ) {
        let provider = self.provider.clone();
        // increment
        *self.req_per_block.entry(block_number).or_default() += 1;

        match ex_type {
            StaticBindingsDb::UniswapV2 | StaticBindingsDb::SushiSwapV2 => {
                self.pool_load_futures.push(Box::pin(async move {
                    // we want end of last block state so that when the new state transition is
                    // applied, the state is still correct
                    let (pool, res) = if let Ok(pool) = UniswapV2Pool::new_load_on_block(
                        address,
                        provider.clone(),
                        block_number - 1,
                    )
                    .await
                    {
                        (pool, LoadResult::Ok)
                    } else {
                        (
                            UniswapV2Pool::new_load_on_block(address, provider, block_number)
                                .await
                                .map_err(|e| {
                                    (address, StaticBindingsDb::UniswapV2, block_number, e)
                                })?,
                            LoadResult::PoolInitOnBlock,
                        )
                    };

                    Ok((
                        block_number,
                        address,
                        PoolState::new(crate::types::PoolVariants::UniswapV2(pool)),
                        res,
                    ))
                }))
            }
            StaticBindingsDb::UniswapV3 | StaticBindingsDb::SushiSwapV3 => {
                self.pool_load_futures.push(Box::pin(async move {
                    // we want end of last block state so that when the new state transition is
                    // applied, the state is still correct
                    let (pool, res) = if let Ok(pool) =
                        UniswapV3Pool::new_from_address(address, block_number - 1, provider.clone())
                            .await
                    {
                        (pool, LoadResult::Ok)
                    } else {
                        (
                            UniswapV3Pool::new_from_address(address, block_number, provider)
                                .await
                                .map_err(|e| {
                                    (address, StaticBindingsDb::UniswapV3, block_number, e)
                                })?,
                            LoadResult::PoolInitOnBlock,
                        )
                    };

                    Ok((
                        block_number,
                        address,
                        PoolState::new(crate::types::PoolVariants::UniswapV3(pool)),
                        res,
                    ))
                }));
            }
            rest @ _ => {
                error!(exchange=?ex_type, "no state updater is build for");
            }
        }
    }

    pub fn is_loading(&self, k: &Address) -> bool {
        self.pool_buf.contains(k)
    }

    pub fn is_empty(&self) -> bool {
        self.pool_buf.is_empty()
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

                    self.pool_buf.remove(&addr);
                    let res = LazyResult { block, state: Some(state), load_result: load };
                    return Poll::Ready(Some(res))
                }
                Err((address, dex, block, e)) => {
                    if let Entry::Occupied(mut o) = self.req_per_block.entry(block) {
                        *(o.get_mut()) -= 1;
                    }

                    let res = LazyResult { state: None, block, load_result: LoadResult::Err };
                    return Poll::Ready(Some(res))
                }
            }
        } else {
            Poll::Pending
        }
    }
}
