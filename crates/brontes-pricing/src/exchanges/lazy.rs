use std::{pin::Pin, sync::Arc, task::Poll};

use alloy_primitives::Address;
use brontes_types::{traits::TracingProvider, Dexes};
use futures::{stream::FuturesUnordered, Future, Stream, StreamExt};
use reth_primitives::revm_primitives::HashMap;
use tracing::error;

use crate::{types::PoolState, uniswap_v2::UniswapV2Pool, uniswap_v3::UniswapV3Pool, PoolUpdate};

pub struct LazyExchangeLoader<T: TracingProvider> {
    provider:          Arc<T>,
    pool_buf:          HashMap<Address, Vec<PoolUpdate>>,
    pool_load_futures: FuturesUnordered<Pin<Box<dyn Future<Output = (Address, PoolState)>>>>,
}

impl<T: TracingProvider> LazyExchangeLoader<T> {
    pub fn new(provider: Arc<T>) -> Self {
        Self {
            pool_buf: HashMap::default(),
            pool_load_futures: FuturesUnordered::default(),
            provider,
        }
    }

    pub fn lazy_load_exchange(&mut self, address: Address, block_number: u64, ex_type: Dexes) {
        let provider = self.provider.clone();
        match ex_type {
            Dexes::UniswapV2 => self.pool_load_futures.push(Box::pin(async move {
                let pool = UniswapV2Pool::new_load_on_block(address, provider, block_number)
                    .await
                    .unwrap();
                (address, PoolState::new(crate::types::PoolVariants::UniswapV2(pool)))
            })),
            Dexes::UniswapV3 => {
                self.pool_load_futures.push(Box::pin(async move {
                    let pool = UniswapV3Pool::new_from_address(address, block_number, provider)
                        .await
                        .unwrap();
                    (address, PoolState::new(crate::types::PoolVariants::UniswapV3(pool)))
                }));
            }
            rest @ _ => {
                error!(exchange =?ex_type, "no state updater is build for");
            }
        }

        todo!()
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
        if let Poll::Ready(Some((pool, state))) = self.pool_load_futures.poll_next_unpin(cx) {
            let buf = self.pool_buf.remove(&pool).unwrap();
            return Poll::Ready(Some((state, buf)))
        } else {
            Poll::Pending
        }
    }
}
