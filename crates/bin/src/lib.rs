use futures::{future::join_all, Future, FutureExt, StreamExt};
use poirot_classifer::classifer::Classifier;
use poirot_core::decoding::Parser;
use poirot_inspect::Inspector;
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

pub const PROMETHEUS_ENDPOINT_IP: [u8; 4] = [127u8, 0u8, 0u8, 1u8];
pub const PROMETHEUS_ENDPOINT_PORT: u16 = 6423;

type InspectorFut<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

pub(crate) struct Poirot<'a> {
    parser: Parser,
    classifier: Classifier,
    // do we care enough to enum dispatch this?
    inspectors: &'a [&'a Box<dyn Inspector + Send + Sync>],
    inspector_task: Option<InspectorFut<'a>>,
}

impl<'a> Poirot<'a> {
    pub(crate) fn new(
        parser: Parser,
        classifier: Classifier,
        inspectors: &'a [&'a Box<dyn Inspector + Send + Sync>],
    ) -> Self {
        Self { parser, classifier, inspectors, inspector_task: None }
    }

    /// returns false if we are already tracing a block
    pub(crate) fn trace_block(&self, block_num: u64) -> bool {
        if self.inspector_task.is_some() {
            return false
        }
        self.parser.execute(block_num);

        true
    }

    fn on_inspectors_finish(&mut self, _data: ()) {}
}

impl<'a> Future for Poirot<'a> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut iters = 1024;
        loop {
            while let Poll::Ready(val) = self.parser.poll_next_unpin(cx) {
                let Some(traces) = val else { return Poll::Ready(()) };
                let tree = Arc::new(self.classifier.build_tree(traces));
                let inspectors = self.inspectors;

                let fut = Box::pin(async move {
                    join_all(
                        inspectors.iter().map(|i| i.process_tree(tree.clone())).collect::<Vec<_>>(),
                    )
                    .await;
                }) as InspectorFut<'a>;

                self.inspector_task = Some(fut);
            }

            if let Some(mut fut) = self.inspector_task.take() {
                match fut.poll_unpin(cx) {
                    Poll::Pending => self.inspector_task = Some(fut),
                    Poll::Ready(fut) => self.on_inspectors_finish(fut),
                }
            }

            iters -= 1;
            if iters == 0 {
                cx.waker().wake_by_ref();
                break
            }
        }

        Poll::Pending
    }
}
