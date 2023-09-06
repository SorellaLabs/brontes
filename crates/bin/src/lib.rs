use poirot_core::decoding::{Parser, TypeToParse};
use std::task::Poll;
pub mod prometheus_exporter;
use futures::Future;
use poirot_normalizer::normalized_actions::NormalizedAction;
use poirot_normalizer::tree::TimeTree;
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

    pub(crate) fn trace_block(&self, block_num: u64) {
        self.parser.execute(TypeToParse::Block(block_num))
    }
}

impl<V> Future for Poirot<V>
where
    V: NormalizedAction + Unpin,
{
    type Output = ();

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        loop {
            while let Some(block_traces) = this.parser.unp {}
        }
        Poll::Pending
    }
}
