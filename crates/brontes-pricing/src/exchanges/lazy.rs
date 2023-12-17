use std::{collections::VecDeque, pin::Pin, task::Poll};

use alloy_primitives::Address;
use futures::{stream::FuturesUnordered, Future, FutureExt, Stream, StreamExt};
use reth_primitives::revm_primitives::HashMap;

use crate::{types::PoolState, PoolUpdate};

pub struct LazyExchangeLoader {
    pool_buf:          HashMap<Address, Vec<PoolUpdate>>,
    pool_load_futures: FuturesUnordered<Pin<Box<dyn Future<Output = (Address, PoolState)>>>>,
}

impl LazyExchangeLoader {
    pub fn new() -> Self {
        Self {
            pool_buf:          HashMap::default(),
            pool_load_futures: FuturesUnordered::default(),
        }
    }

    pub fn lazy_load_exchange(&mut self, address: Address, block_number: u64, ex_type: ()) {
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

impl Stream for LazyExchangeLoader {
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
