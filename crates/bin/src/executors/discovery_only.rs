use std::{
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use brontes_classifier::{discovery_only::DiscoveryOnlyClassifier, Classifier};
use brontes_core::decoding::{Parser, TracingProvider};
use brontes_database::{
    clickhouse::ClickhouseHandle,
    libmdbx::{DBWriter, LibmdbxReader},
};
use brontes_inspect::Inspector;
use brontes_metrics::range::GlobalRangeMetrics;
use brontes_types::{db::metadata::Metadata, normalized_actions::Action, tree::BlockTree};
use futures::{pin_mut, stream::FuturesUnordered, Future, StreamExt};
use reth_tasks::shutdown::GracefulShutdown;
use tracing::debug;

use super::shared::state_collector::StateCollector;
use crate::{executors::ProgressBar, Processor};

const MAX_PENDING_TREE_BUILDING: usize = 10;

/// only runs discovery
pub struct DiscoveryExecutor<T: TracingProvider, DB: DBWriter + LibmdbxReader>
{
    current_block: u64,
    end_block:     u64,
    db:            &'static DB,
    parser:        &'static Parser<T, DB>,
    classifier:    DiscoveryOnlyClassifier<'static, T, DB>,
    running:       FuturesUnordered<Pin<Box<dyn Future<Output = ()> + Send>>>,
    progress_bar:  ProgressBar,
}

impl<T: TracingProvider, DB: LibmdbxReader + DBWriter>
    DiscoveryExecutor<T, DB, CH>
{
    pub fn new(
        start_block: u64,
        end_block: u64,
        db: &'static DB,
        parser: &'static Parser<T, DB>,
        progress_bar: ProgressBar,
    ) -> Self {
        let classifier = DiscoveryOnlyClassifier::new(db, parser.get_tracer());
        Self {
            progress_bar,
            current_block: start_block,
            end_block,
            db,
            parser,
            classifier,
            running: FuturesUnordered::default(),
        }
    }

    pub async fn run_until_graceful_shutdown(self, shutdown: GracefulShutdown) {
        let data_batching = self;
        pin_mut!(data_batching, shutdown);

        let mut graceful_guard = None;
        tokio::select! {
            _ = &mut data_batching => {
            },
            guard = shutdown => {
                graceful_guard = Some(guard);
            },
        }
        while (data_batching.running.next().await).is_some() {}

        drop(graceful_guard);
    }

    async fn process_next(
        block: u64,
        parser: &'static Parser<T, DB>,
        classifier: DiscoveryOnlyClassifier<T, DB>,
    ) {
        if let Ok(Some((traces, header))) = parser.execute_no_metrics(block).await {
            classifier.run_discovery(traces, header).await
        }
    }
}

impl<T: TracingProvider, DB: LibmdbxReader + DBWriter> Future
    for DiscoveryExecutor<T, DB, CH>
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.current_block != self.end_block && self.running.len() < MAX_PENDING_TREE_BUILDING {
            cx.waker().wake_by_ref();
            let fut = Box::pin(Self::process_next(
                self.current_block,
                self.parser,
                self.classifier.clone(),
            ));
            self.current_block += 1;
        }

        while match self.running.poll_next_unpin(cx) {
            Poll::Ready(Some(_)) => {
                self.progress_bar.inc(1);
                true
            }
            Poll::Pending => false,
            Poll::Ready(None) => return Poll::Ready(()),
        } {}

        Poll::Pending
    }
}
