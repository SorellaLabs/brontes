use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll}
};

use futures::{
    future::{join_all, JoinAll},
    Future, FutureExt
};
use poirot_classifer::classifer::Classifier;
use poirot_core::decoding::Parser;
use poirot_inspect::{ClassifiedMev, Inspector};
use poirot_labeller::{Labeller, Metadata};
use poirot_types::{normalized_actions::Actions, structured_trace::TxTrace, tree::TimeTree};
use reth_primitives::Header;
use tokio::task::JoinError;

pub const PROMETHEUS_ENDPOINT_IP: [u8; 4] = [127u8, 0u8, 0u8, 1u8];
pub const PROMETHEUS_ENDPOINT_PORT: u16 = 6423;

type InspectorFut<'a> = JoinAll<Pin<Box<dyn Future<Output = Vec<ClassifiedMev>> + Send + 'a>>>;

type CollectionFut<'a> = Pin<
    Box<
        dyn Future<Output = (Result<Option<(Vec<TxTrace>, Header)>, JoinError>, Metadata)>
            + Send
            + 'a
    >
>;

pub struct Poirot<'a, const N: usize> {
    current_block: u64,
    parser:        Parser,
    classifier:    Classifier,
    labeller:      Labeller<'a>,

    inspectors: &'a [&'a Box<dyn Inspector>; N],

    // pending future data
    inspector_task:  Option<InspectorFut<'a>>,
    classifier_data: Option<CollectionFut<'a>>
}

impl<'a, const N: usize> Poirot<'a, N> {
    pub fn new(
        parser: Parser,
        labeller: Labeller<'a>,
        classifier: Classifier,
        inspectors: &'a [&'a Box<dyn Inspector>; N],
        init_block: u64
    ) -> Self {
        Self {
            parser,
            labeller,
            classifier,
            inspectors,
            inspector_task: None,
            current_block: init_block,
            classifier_data: None
        }
    }

    fn start_new_block(&self) -> bool {
        self.classifier_data.is_none() && self.inspector_task.is_none()
    }

    fn start_collection(&mut self) {
        let Ok(Some(hash)) = self
            .parser
            .get_block_hash_for_number(self.current_block + 1)
        else {
            // no new block ready
            return
        };
        self.current_block += 1;

        let parser_fut = self.parser.execute(self.current_block);
        let labeller_fut = self.labeller.get_metadata(self.current_block, hash.into());

        self.classifier_data = Some(Box::pin(async { (parser_fut.await, labeller_fut.await) }));
    }

    fn start_inspecting(&mut self, tree: Arc<TimeTree<Actions>>, metadata: Arc<Metadata>) {
        self.inspector_task = Some(join_all(
            self.inspectors
                .iter()
                .map(|inspector| inspector.process_tree(tree.clone(), metadata.clone()))
        ) as InspectorFut<'a>);
    }

    fn on_inspectors_finish(&mut self, _data: Vec<Vec<ClassifiedMev>>) {
        todo!()
    }

    fn progress_futures(&mut self, cx: &mut Context<'_>) {
        if let Some(mut collection_fut) = self.classifier_data.take() {
            match collection_fut.poll_unpin(cx) {
                Poll::Ready((parser_data, labeller_data)) => {
                    let (traces, header) = parser_data.unwrap().unwrap();
                    let tree = self.classifier.build_tree(traces, header, &labeller_data);
                    self.start_inspecting(tree.into(), labeller_data.into());
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

impl<'a, const N: usize> Future for Poirot<'a, N> {
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
