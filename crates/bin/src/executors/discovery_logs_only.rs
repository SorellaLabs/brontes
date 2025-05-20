use std::{
    collections::HashMap,
    pin::Pin,
    task::{Context, Poll},
};

use brontes_classifier::discovery_logs_only::DiscoveryLogsOnlyClassifier;
use brontes_core::decoding::{LogParser, LogProvider};
use brontes_database::libmdbx::{DBWriter, LibmdbxReader};
use brontes_types::Protocol;
use futures::{pin_mut, stream::FuturesUnordered, Future, StreamExt};
use reth_tasks::shutdown::GracefulShutdown;

use crate::executors::ProgressBar;

const MAX_PENDING_TREE_BUILDING: usize = 5;

/// only runs discovery
pub struct DiscoveryLogsExecutor<T: LogProvider, DB: DBWriter + LibmdbxReader> {
    current_block: u64,
    end_block:     u64,
    parser:        &'static LogParser<T, DB>,
    classifier:    DiscoveryLogsOnlyClassifier<'static, DB>,
    running:       FuturesUnordered<Pin<Box<dyn Future<Output = ()> + Send>>>,
    progress_bar:  ProgressBar,
}

impl<T: LogProvider, DB: LibmdbxReader + DBWriter> DiscoveryLogsExecutor<T, DB> {
    pub fn new(
        start_block: u64,
        end_block: u64,
        db: &'static DB,
        parser: &'static LogParser<T, DB>,
        progress_bar: ProgressBar,
    ) -> Self {
        let classifier = DiscoveryLogsOnlyClassifier::new(db);
        Self {
            progress_bar,
            current_block: start_block,
            end_block,
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
        start_block: u64,
        end_block: u64,
        parser: &'static LogParser<T, DB>,
        classifier: DiscoveryLogsOnlyClassifier<'static, DB>,
    ) {
        if let Some((block_number, protocol_to_logs)) =
            parser.execute_discovery(start_block, end_block).await
        {
            let data: HashMap<Protocol, Vec<alloy_primitives::Log>> =
                protocol_to_logs
                    .iter()
                    .fold(HashMap::new(), |mut acc, (protocol, logs)| {
                        let plogs: Vec<alloy_primitives::Log> = logs.iter().map(|log| log.inner.clone()).collect();
                        acc.insert(*protocol, plogs);
                        acc
                    });

            classifier.run_discovery(block_number, data).await
        }
    }
}

impl<T: LogProvider, DB: LibmdbxReader + DBWriter> Future for DiscoveryLogsExecutor<T, DB> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.current_block != self.end_block && self.running.len() < MAX_PENDING_TREE_BUILDING {
            cx.waker().wake_by_ref();
            let fut = Box::pin(Self::process_next(
                self.current_block,
                self.end_block,
                self.parser,
                self.classifier.clone(),
            ));
            self.running.push(fut);

            self.current_block = self.end_block;
        }

        while match self.running.poll_next_unpin(cx) {
            Poll::Ready(Some(_)) => {
                self.progress_bar.inc(1);
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
