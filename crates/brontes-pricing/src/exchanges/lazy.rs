use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    pin::Pin,
    sync::Arc,
    task::Poll,
};

use alloy_primitives::Address;
use brontes_types::{exchanges::StaticBindingsDb, extra_processing::Pair, traits::TracingProvider};
use futures::{
    future::BoxFuture,
    stream::{FuturesOrdered, FuturesUnordered},
    Future, Stream, StreamExt,
};
use tracing::{error, info};

use crate::{
    errors::AmmError, graphs::PoolPairInfoDirection, types::PoolState, uniswap_v2::UniswapV2Pool,
    uniswap_v3::UniswapV3Pool, PoolUpdate,
};

type PoolFetchError = (Address, StaticBindingsDb, u64, AmmError);
type PoolFetchSuccess = (u64, Address, PoolState, LoadResult);

pub enum LoadResult {
    Ok,
    /// because we back query 1 block. if a pool is created at the current
    /// block, this will error. because of this we need to signal this case
    /// to the pricing engine so that we don't apply any state transitions
    /// for this block as it will cause incorrect data
    PoolInitOnBlock,
    Err,
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

/// Deals with the lazy loading of new exchange state, and tracks loading of new
/// state for a given block.
pub struct LazyExchangeLoader<T: TracingProvider> {
    provider:          Arc<T>,
    pool_load_futures: FuturesOrdered<BoxFuture<'static, Result<PoolFetchSuccess, PoolFetchError>>>,
    /// addresses currently being processed.
    pool_buf:          HashSet<Address>,
    /// requests we are processing for a given block.
    req_per_block:     HashMap<u64, u64>,
}

impl<T: TracingProvider> LazyExchangeLoader<T> {
    pub fn new(provider: Arc<T>) -> Self {
        Self {
            pool_buf: HashSet::default(),
            pool_load_futures: FuturesOrdered::default(),
            provider,
            req_per_block: HashMap::default(),
        }
    }

    pub fn requests_for_block(&self, block: &u64) -> u64 {
        self.req_per_block.get(block).copied().unwrap_or(0)
    }

    pub fn lazy_load_exchange(
        &mut self,
        address: Address,
        block_number: u64,
        ex_type: StaticBindingsDb,
    ) {
        info!(%block_number, dex=%ex_type,"loading exchange");
        let provider = self.provider.clone();
        *self.req_per_block.entry(block_number).or_default() += 1;
        self.pool_buf.insert(address);

        match ex_type {
            StaticBindingsDb::UniswapV2 | StaticBindingsDb::SushiSwapV2 => {
                self.pool_load_futures.push_back(Box::pin(async move {
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
                self.pool_load_futures.push_back(Box::pin(async move {
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
            rest => {
                error!(exchange=?ex_type, "no state updater is build for");
            }
        }
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

                    self.pool_buf.remove(&addr);
                    let res = LazyResult { block, state: Some(state), load_result: load };
                    Poll::Ready(Some(res))
                }
                Err((address, dex, block, e)) => {
                    if let Entry::Occupied(mut o) = self.req_per_block.entry(block) {
                        *(o.get_mut()) -= 1;
                    }

                    self.pool_buf.remove(&address);
                    let res = LazyResult { state: None, block, load_result: LoadResult::Err };
                    Poll::Ready(Some(res))
                }
            }
        } else {
            Poll::Pending
        }
    }
}
