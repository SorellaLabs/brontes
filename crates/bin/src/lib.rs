use poirot_core::{
    decoding::{Parser, TypeToParse},
    structured_trace::TxTrace,
};
use std::task::Poll;
pub mod prometheus_exporter;
use futures::{Future, StreamExt};
use poirot_types::{normalized_actions::NormalizedAction, tree::TimeTree};
use std::{pin::Pin, task::Context};

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

    fn build_tree(&self, traces: Vec<TxTrace>) {}
}

impl<V> Future for Poirot<V>
where
    V: NormalizedAction + Unpin,
{
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        let mut iters = 1024;
        loop {
            while let Poll::Ready(val) = this.parser.poll_next_unpin(cx) {
                if val.is_none() {
                    return Poll::Ready(())
                }

                match val.unwrap() {
                    Some(block_traces) => (),
                    None => (),
                }
            }
            iters -= 1;
            if iters == 0 {
                cx.waker().wake_by_ref();
                break
            }
        }

        return Poll::Pending
    }
}
