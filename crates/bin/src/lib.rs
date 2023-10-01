use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures::{Future, FutureExt, StreamExt};
use poirot_classifier::Classifier;
use poirot_core::decoding::Parser;
use poirot_database::{database::Database, Metadata};
use poirot_inspect::composer::Composer;
use poirot_types::{
    classified_mev::{ClassifiedMev, MevBlock, SpecificMev},
    structured_trace::TxTrace,
};
use reth_primitives::Header;
use tokio::task::JoinError;

pub const PROMETHEUS_ENDPOINT_IP: [u8; 4] = [127u8, 0u8, 0u8, 1u8];
pub const PROMETHEUS_ENDPOINT_PORT: u16 = 6423;

type CollectionFut<'a> = Pin<
    Box<
        dyn Future<Output = (Result<Option<(Vec<TxTrace>, Header)>, JoinError>, Metadata)>
            + Send
            + 'a,
    >,
>;

pub struct Poirot<'inspector, 'db, const N: usize> {
    current_block: u64,
    parser:        Parser,
    classifier:    Classifier,
    database:      &'db Database,
    composer:      Composer<'inspector, N>,

    // pending future data
    classifier_future: Option<CollectionFut<'db>>,
    // pending insertion data
    insertion_future:  Option<Pin<Box<dyn Future<Output = ()> + Send + Sync + 'db>>>,
}

impl<'inspector, 'db, const N: usize> Poirot<'inspector, 'db, N> {
    pub fn new(
        parser: Parser,
        database: &'db Database,
        classifier: Classifier,
        composer: Composer<'inspector, N>,
        init_block: u64,
    ) -> Self {
        Self {
            parser,
            database,
            classifier,
            composer,
            current_block: init_block,
            classifier_future: None,
            insertion_future: None,
        }
    }

    fn start_new_block(&self) -> bool {
        self.classifier_future.is_none()
            && !self.composer.is_processing()
            && self.insertion_future.is_none()
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
        let labeller_fut = self.database.get_metadata(self.current_block, hash.into());

        self.classifier_future = Some(Box::pin(async { (parser_fut.await, labeller_fut.await) }));
    }

    fn on_inspectors_finish(
        &mut self,
        results: (MevBlock, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>),
    ) {
        self.insertion_future =
            Some(Box::pin(self.database.insert_classified_data(results.0, results.1)));
    }

    fn progress_futures(&mut self, cx: &mut Context<'_>) {
        if let Some(mut collection_fut) = self.classifier_future.take() {
            match collection_fut.poll_unpin(cx) {
                Poll::Ready((parser_data, labeller_data)) => {
                    let (traces, header) = parser_data.unwrap().unwrap();
                    let tree = self.classifier.build_tree(traces, header, &labeller_data);
                    self.composer.on_new_tree(tree.into(), labeller_data.into());
                }
                Poll::Pending => {
                    self.classifier_future = Some(collection_fut);
                    return
                }
            }
        }

        if let Poll::Ready(Some(data)) = self.composer.poll_next_unpin(cx) {
            self.on_inspectors_finish(data);
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

impl<const N: usize> Future for Poirot<'_, '_, N> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // This loop drives the entire state of network and does a lot of work.
        // Under heavy load (many messages/events), data may arrive faster than it can
        // be processed (incoming messages/requests -> events), and it is
        // possible that more data has already arrived by the time an internal
        // event is processed. Which could turn this loop into a busy loop.
        // Without yielding back to the executor, it can starve other tasks waiting on
        // that executor to execute them, or drive underlying resources To prevent this,
        // we preemptively return control when the `budget` is exhausted. The
        // value itself is chosen somewhat arbitrarily, it is high enough so the
        // swarm can make meaningful progress but low enough that this loop does
        // not starve other tasks for too long. If the budget is exhausted we
        // manually yield back control to the (coop) scheduler. This manual yield point should prevent situations where polling appears to be frozen. See also <https://tokio.rs/blog/2020-04-preemption>
        // And tokio's docs on cooperative scheduling <https://docs.rs/tokio/latest/tokio/task/#cooperative-scheduling>

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
