use futures::{
    future::{join_all, JoinAll},
    Future, FutureExt,
};
use malachite::Rational;
use poirot_classifer::classifer::Classifier;
use poirot_core::decoding::Parser;
use poirot_inspect::Inspector;
use poirot_types::{normalized_actions::Actions, structured_trace::TxTrace, tree::TimeTree};
use reth_primitives::Header;
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll}, thread::current,
};
use tokio::task::JoinError;
use poirot_labeller::{Labeller, database::Metadata};
pub const PROMETHEUS_ENDPOINT_IP: [u8; 4] = [127u8, 0u8, 0u8, 1u8];
pub const PROMETHEUS_ENDPOINT_PORT: u16 = 6423;

type InspectorFut<'a> = JoinAll<Pin<Box<dyn Future<Output = ()> + Send + 'a>>>;

type CollectionFut = Pin<
    Box<
        dyn Future<Output = (Result<Option<(Vec<TxTrace>, Header)>, JoinError>, Metadata)>
            + Send
            + 'static,
    >,
>;

pub struct Poirot<'a> {
    current_block: u64,
    parser: Parser,
    classifier: Classifier,
    labeller: Labeller,

    inspectors: &'a [&'a Box<dyn Inspector + Send + Sync>],

    // pending future data
    inspector_task: Option<InspectorFut<'a>>,
    classifier_data: Option<CollectionFut>,
}

impl<'a> Poirot<'a> {
    pub fn new(
        parser: Parser,
        labeller: Labeller,
        classifier: Classifier,
        inspectors: &'a [&'a Box<dyn Inspector + Send + Sync>],
        init_block: u64,
    ) -> Self {
        Self {
            parser,
            labeller,
            classifier,
            inspectors,
            inspector_task: None,
            current_block: init_block,
            classifier_data: None,
        }
    }

    fn start_new_block(&self) -> bool {
        self.classifier_data.is_none() && self.inspector_task.is_none()
    }

    fn start_collection(&mut self) {
        let Ok(Some(hash)) = self.parser.get_block_hash_for_number(self.current_block + 1) else {
            // no new block ready
            return
        };
        self.current_block += 1;

        let parser_fut = self.parser.execute(self.current_block);
        // placeholder for ludwigs shit
        let labeller_fut = self.labeller.client.get_metadata(self.current_block, hash.into());

        self.classifier_data = Some(Box::pin(async { (parser_fut.await, labeller_fut.await) }));
    }

    fn start_inspecting(&mut self, tree: Arc<TimeTree<Actions>>) {
        self.inspector_task = Some(join_all(
            self.inspectors.iter().map(|inspector| inspector.process_tree(tree.clone())),
        ) as InspectorFut<'a>);
    }

    fn on_inspectors_finish(&mut self, _data: Vec<()>) {}
    
    fn progress_futures(&mut self, cx: &mut Context<'_>) {
        if let Some(mut collection_fut) = self.classifier_data.take() {
            match collection_fut.poll_unpin(cx) {
                Poll::Ready((parser_data, labeller_data)) => {
                    let (traces, header) = parser_data.unwrap().unwrap();
                    let tree = self.classifier.build_tree(traces, header, labeller_data);
                    self.start_inspecting(tree.into());
                }
                Poll::Pending => {
                    self.classifier_data = Some(collection_fut);
                    return
                }
            }
        }

        if let Some(mut inspector_results) = self.inspector_task.take() {
            match inspector_results.poll_unpin(cx) {
                Poll::Ready(data) => self.on_inspectors_finish(data),
                Poll::Pending => {
                    self.inspector_task = Some(inspector_results);
                }
            }
        }
    }
}

impl<'a> Future for Poirot<'a> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut iters = 1024;
        loop {
            if self.start_new_block() {
                self.start_collection();
            }

            self.progress_futures(cx);

            iters -= 1;
            if iters == 0 {
                cx.waker().wake_by_ref();
                break
            }
        }

        Poll::Pending
    }
}
