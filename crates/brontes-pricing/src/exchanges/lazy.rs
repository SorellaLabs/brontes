use futures::Stream;

use crate::types::PoolState;

pub struct LazyExchangeLoader {}

impl LazyExchangeLoader {
    pub fn lazy_load_exchange(&mut self) {}
}

impl Stream for LazyExchangeLoader {
    type Item = PoolState;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        todo!()
    }
}
