use poirot_core::decoding::Parser;
use std::task::Poll;
pub mod prometheus_exporter;
use futures::Future;
use poirot_normalizer::{normalized_actions::NormalizedAction, tree::TimeTree};
use std::task::Context;

pub const PROMETHEUS_ENDPOINT_IP: [u8; 4] = [127u8, 0u8, 0u8, 1u8];
pub const PROMETHEUS_ENDPOINT_PORT: u16 = 6423;

pub(crate) struct Poirot<V: NormalizedAction> {
    parser: Parser,
    tree: TimeTree<V>,
}

impl<V: NormalizedAction> Poirot<V> {
    pub(crate) fn new(parser: Parser, tree: TimeTree<V>) -> Self {
        Self { parser, tree }
    }
}

impl<V> Future for Poirot<V>
where
    V: NormalizedAction + Unpin,
{
    type Output = ();

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        Poll::Pending
    }
}
