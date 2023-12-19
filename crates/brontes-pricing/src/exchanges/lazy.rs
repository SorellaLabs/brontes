use std::{
    collections::{hash_map::Entry, HashMap},
    pin::Pin,
    sync::Arc,
    task::Poll,
};

use alloy_primitives::Address;
use brontes_types::{exchanges::StaticBindingsDb, traits::TracingProvider};
use futures::{stream::FuturesUnordered, Future, Stream, StreamExt};
use tracing::{error, info};

use crate::{types::PoolState, uniswap_v2::UniswapV2Pool, uniswap_v3::UniswapV3Pool, PoolUpdate};

pub struct LazyExchangeLoader<T: TracingProvider> {
    provider:          Arc<T>,
    pool_buf:          HashMap<Address, Vec<PoolUpdate>>,
    pool_load_futures:
        FuturesUnordered<Pin<Box<dyn Future<Output = (u64, Address, PoolState)> + Send>>>,
    // the different blocks that we are currently fetching
    req_per_block:     HashMap<u64, u64>,
}

impl<T: TracingProvider> LazyExchangeLoader<T> {
    pub fn new(provider: Arc<T>) -> Self {
        Self {
            pool_buf: HashMap::default(),
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
                    let pool = UniswapV2Pool::new_load_on_block(address, provider, block_number)
                        .await
                        .unwrap();
                    (
                        block_number,
                        address,
                        PoolState::new(crate::types::PoolVariants::UniswapV2(pool)),
                    )
                }))
            }
            StaticBindingsDb::UniswapV3 | StaticBindingsDb::SushiSwapV3 => {
                self.pool_load_futures.push(Box::pin(async move {
                    let pool = UniswapV3Pool::new_from_address(address, block_number, provider)
                        .await
                        .unwrap();
                    (
                        block_number,
                        address,
                        PoolState::new(crate::types::PoolVariants::UniswapV3(pool)),
                    )
                }));
            }
            rest @ _ => {
                error!(exchange =?ex_type, "no state updater is build for");
            }
        }
    }

    pub fn is_loading(&self, k: &Address) -> bool {
        self.pool_buf.contains_key(k)
    }

    pub fn buffer_update(&mut self, k: &Address, update: PoolUpdate) {
        self.pool_buf
            .get_mut(k)
            .expect("buffered lazy exchange when no exchange future was found")
            .push(update);
    }

    pub fn is_empty(&self) -> bool {
        self.pool_buf.is_empty()
    }
}

impl<T: TracingProvider> Stream for LazyExchangeLoader<T> {
    type Item = (PoolState, Vec<PoolUpdate>);

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        if let Poll::Ready(Some((block, pool, state))) = self.pool_load_futures.poll_next_unpin(cx)
        {
            info!("loaded pool");
            if let Entry::Occupied(mut o) = self.req_per_block.entry(block) {
                *(o.get_mut()) -= 1;
            } else {
                unreachable!()
            }

            let buf = self.pool_buf.remove(&pool).unwrap_or(vec![]);
            return Poll::Ready(Some((state, buf)))
        } else {
            Poll::Pending
        }
    }
}
