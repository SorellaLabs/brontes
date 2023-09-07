use poirot_core::decoding::{Parser, TypeToParse};
use std::task::Poll;
pub mod prometheus_exporter;
use futures::{Future, StreamExt};
use std::{pin::Pin, task::Context};

pub const PROMETHEUS_ENDPOINT_IP: [u8; 4] = [127u8, 0u8, 0u8, 1u8];
pub const PROMETHEUS_ENDPOINT_PORT: u16 = 6423;

pub(crate) struct Poirot {
    parser: Parser,
}

impl Poirot {
    pub(crate) fn new(parser: Parser) -> Self {
        Self { parser }
    }

    pub(crate) fn trace_block(&self, block_num: u64) {
        self.parser.execute(TypeToParse::Block(block_num))
    }
}

impl Future for Poirot {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        let mut iters = 1024;
        loop {
            while let Poll::Ready(val) = this.parser.poll_next_unpin(cx) {
                let tree = if let Some(t) = val { t } else { return Poll::Ready(()) };
            }
            iters -= 1;
            if iters == 0 {
                cx.waker().wake_by_ref();
                break;
            }
        }

        return Poll::Pending;
    }
}
