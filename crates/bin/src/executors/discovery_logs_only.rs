use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use brontes_classifier::discovery_logs_only::DiscoveryLogsOnlyClassifier;
use brontes_core::decoding::{LogParser, LogProvider};
use brontes_database::libmdbx::{DBWriter, LibmdbxReader};
use futures::{pin_mut, stream::FuturesUnordered, Future, StreamExt};
use reth_tasks::shutdown::GracefulShutdown;
use tokio::time::{sleep_until, Instant, Sleep};

use crate::executors::ProgressBar;

const MAX_PENDING_TREE_BUILDING: usize = 5;

/// only runs discovery
pub struct DiscoveryLogsExecutor<T: LogProvider, DB: DBWriter + LibmdbxReader> {
    current_block: u64,
    end_block:     u64,
    range_size:    usize,
    parser:        &'static LogParser<T, DB>,
    classifier:    DiscoveryLogsOnlyClassifier<'static, DB>,
    running:       FuturesUnordered<Pin<Box<dyn Future<Output = eyre::Result<()>> + Send>>>,
    progress_bar:  ProgressBar,
    sleep:         Pin<Box<Sleep>>,
}

impl<T: LogProvider, DB: LibmdbxReader + DBWriter> DiscoveryLogsExecutor<T, DB> {
    pub fn new(
        start_block: u64,
        end_block: u64,
        range_size: usize,
        db: &'static DB,
        parser: &'static LogParser<T, DB>,
        progress_bar: ProgressBar,
    ) -> Self {
        let classifier = DiscoveryLogsOnlyClassifier::new(db);
        let sleep = Box::pin(sleep_until(Instant::now() + Duration::from_millis(100)));
        Self {
            progress_bar,
            current_block: start_block,
            end_block,
            range_size,
            parser,
            classifier,
            running: FuturesUnordered::default(),
            sleep,
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
        start_block: u64,
        end_block: u64,
        parser: &'static LogParser<T, DB>,
        classifier: DiscoveryLogsOnlyClassifier<'static, DB>,
    ) -> eyre::Result<()> {
        let logs = parser.execute_discovery(start_block, end_block).await?;
        classifier.process_logs(logs).await?;
        Ok(())
    }
}

impl<T: LogProvider, DB: LibmdbxReader + DBWriter> Future for DiscoveryLogsExecutor<T, DB> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Poll the sleep future first
        if let Poll::Ready(_) = self.sleep.as_mut().poll(cx) {
            // Reset the sleep for next time
            self.sleep.set(sleep_until(Instant::now() + Duration::from_millis(500)));
        } else {
            // If sleep is not ready, return Pending
            return Poll::Pending;
        }

        if self.current_block < self.end_block && self.running.len() < MAX_PENDING_TREE_BUILDING {
            cx.waker().wake_by_ref();
            let fut = Box::pin(Self::process_next(
                self.current_block,
                self.current_block + self.range_size as u64,
                self.parser,
                self.classifier.clone(),
            ));
            self.running.push(fut);
            self.current_block = std::cmp::min(self.current_block + self.range_size as u64, self.end_block);
        }

        while match self.running.poll_next_unpin(cx) {
            Poll::Ready(Some(result)) => {
                if result.is_err() {
                    tracing::error!("Error processing logs: {:?}", result);
                }
                self.progress_bar.inc(self.range_size as u64);
                true
            }
            Poll::Pending => false,
            Poll::Ready(None) if self.current_block == self.end_block => return Poll::Ready(()),
            Poll::Ready(None) => {
                cx.waker().wake_by_ref();
                false
            }
        } {}

        Poll::Pending
    }
}
