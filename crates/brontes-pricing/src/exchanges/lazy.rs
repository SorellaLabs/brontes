use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    pin::Pin,
    sync::Arc,
    task::Poll,
};

use alloy_primitives::Address;
use brontes_types::{exchanges::StaticBindingsDb, traits::TracingProvider};
use futures::{future::BoxFuture, stream::FuturesUnordered, Future, Stream, StreamExt};
use tracing::{error, info};

use crate::{
    errors::AmmError, types::PoolState, uniswap_v2::UniswapV2Pool, uniswap_v3::UniswapV3Pool,
    PoolUpdate,
};

type PoolFetchError = (Address, StaticBindingsDb, u64, AmmError);
type PoolFetchSuccess = (u64, Address, PoolState);

pub enum LoadResult {
    // because we back query 1 block. this breaks so we need to instead
    // do a special query
    PoolInitOnBlock,
    PoolDoesNotExistYet,
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
                    let pool =
                        UniswapV2Pool::new_load_on_block(address, provider, block_number - 1)
                            .await
                            .map_err(|e| (address, StaticBindingsDb::UniswapV2, block_number, e))?;
                    Ok((
                        block_number,
                        address,
                        PoolState::new(crate::types::PoolVariants::UniswapV2(pool)),
                    ))
                }))
            }
            StaticBindingsDb::UniswapV3 | StaticBindingsDb::SushiSwapV3 => {
                self.pool_load_futures.push(Box::pin(async move {
                    // we want end of last block state so that when the new state transition is
                    // applied, the state is still correct
                    let pool = UniswapV3Pool::new_from_address(address, block_number - 1, provider)
                        .await
                        .map_err(|e| (address, StaticBindingsDb::UniswapV3, block_number, e))?;
                    Ok((
                        block_number,
                        address,
                        PoolState::new(crate::types::PoolVariants::UniswapV3(pool)),
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
    type Item = PoolState;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        if let Poll::Ready(Some((result))) = self.pool_load_futures.poll_next_unpin(cx) {
            match result {
                Ok((block, addr, state)) => {
                    if let Entry::Occupied(mut o) = self.req_per_block.entry(block) {
                        *(o.get_mut()) -= 1;
                    }

                    self.pool_buf.remove(&addr);
                    return Poll::Ready(Some(state))
                }
                Err((address, dex, block, e)) => {
                    error!(?address, exchange_type=%dex, block_number=block, ?e, "failed to load pool");

                    if let Entry::Occupied(mut o) = self.req_per_block.entry(block) {
                        *(o.get_mut()) -= 1;
                    }

                    return Poll::Pending
                }
            }
        } else {
            Poll::Pending
        }
    }
}
