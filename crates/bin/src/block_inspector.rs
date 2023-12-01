use std::{
    pin::Pin,
    task::{Context, Poll},
};

use brontes_classifier::Classifier;
use brontes_core::decoding::{Parser, TracingProvider};
use brontes_database::{database::Database, Metadata};
use brontes_inspect::{composer::Composer, Inspector};
use brontes_types::{
    classified_mev::{ClassifiedMev, MevBlock, SpecificMev},
    structured_trace::TxTrace,
};
use futures::{join, Future, FutureExt};
use reth_primitives::Header;
use tokio::task::JoinError;
use tracing::info;

type CollectionFut<'a> = Pin<
    Box<
        dyn Future<Output = (Result<Option<(Vec<TxTrace>, Header)>, JoinError>, Metadata)>
            + Send
            + 'a,
    >,
>;

pub struct BlockInspector<'inspector, const N: usize, T: TracingProvider> {
    block_number:      u64,
    parser:            &'inspector Parser<'inspector, T>,
    classifier:        &'inspector Classifier,
    database:          &'inspector Database,
    composer:          Composer<'inspector, N>,
    // pending future data
    classifier_future: Option<CollectionFut<'inspector>>,
    // pending insertion data
    insertion_future:  Option<Pin<Box<dyn Future<Output = ()> + Send + Sync + 'inspector>>>,
}

impl<'inspector, const N: usize, T: TracingProvider> BlockInspector<'inspector, N, T> {
    pub fn new(
        parser: &'inspector Parser<'inspector, T>,
        database: &'inspector Database,
        classifier: &'inspector Classifier,
        inspectors: &'inspector [&'inspector Box<dyn Inspector>; N],
        block_number: u64,
    ) -> Self {
        Self {
            block_number,
            parser,
            database,
            classifier,
            composer: Composer::new(inspectors),
            classifier_future: None,
            insertion_future: None,
        }
    }

    fn start_collection(&mut self) {
        info!(block_number = self.block_number, "starting collection of data");
        let parser_fut = self.parser.execute(self.block_number);
        let labeller_fut = self.database.get_metadata(self.block_number);

        self.classifier_future = Some(Box::pin(async { join!(parser_fut, labeller_fut) }));
    }

    fn on_inspectors_finish(
        &mut self,
        results: (MevBlock, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>),
    ) {
        info!(block_number = self.block_number, results=?results, "inserting the collected results");
        self.insertion_future =
            Some(Box::pin(self.database.insert_classified_data(results.0, results.1)));
    }

    fn progress_futures(&mut self, cx: &mut Context<'_>) {
        if let Some(mut collection_fut) = self.classifier_future.take() {
            match collection_fut.poll_unpin(cx) {
                Poll::Ready((parser_data, labeller_data)) => {
                    let (traces, header) = parser_data.unwrap().unwrap();
                    println!("Got {} traces + header + metadata", traces.len());
                    let tree = self.classifier.build_tree(traces, header, &labeller_data);
                    println!("built tree");
                    self.composer.on_new_tree(tree.into(), labeller_data.into());
                }
                Poll::Pending => {
                    self.classifier_future = Some(collection_fut);
                    return
                }
            }
        }

        if !self.composer.is_finished() {
            if let Poll::Ready(data) = self.composer.poll_unpin(cx) {
                self.on_inspectors_finish(data);
            }
        }

        if let Some(mut insertion_future) = self.insertion_future.take() {
            match insertion_future.poll_unpin(cx) {
                Poll::Ready(_) => {}
                Poll::Pending => {
                    self.insertion_future = Some(insertion_future);
                }
            }
        }
    }
}

impl<const N: usize, T: TracingProvider> Future for BlockInspector<'_, N, T> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // If the classifier_future is None (not started yet), start the collection
        // phase
        if self.classifier_future.is_none()
            && !self.composer.is_processing()
            && self.insertion_future.is_none()
        {
            self.start_collection();
        }

        self.progress_futures(cx);

        // Decide when to finish the BlockInspector's future.
        // Finish when both classifier and insertion futures are done.
        if self.classifier_future.is_none()
            && !self.composer.is_processing()
            && self.insertion_future.is_none()
        {
            info!(block_number = self.block_number, "finished inspecting block");
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
