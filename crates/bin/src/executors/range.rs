use std::{
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use brontes_core::decoding::TracingProvider;
use brontes_database::{
    clickhouse::ClickhouseHandle,
    libmdbx::{DBWriter, LibmdbxReader},
};
use brontes_inspect::Inspector;
use brontes_metrics::range::{GlobalRangeMetrics, RangeMetrics};
use brontes_types::{db::metadata::Metadata, normalized_actions::Action, tree::BlockTree};
use futures::{pin_mut, stream::FuturesUnordered, Future, StreamExt};
use reth_tasks::shutdown::GracefulShutdown;
use tracing::debug;

use super::shared::state_collector::StateCollector;
use crate::{executors::ProgressBar, Processor};

pub struct RangeExecutorWithPricing<
    T: TracingProvider,
    DB: DBWriter + LibmdbxReader,
    CH: ClickhouseHandle,
    P: Processor,
> {
    collector:      StateCollector<T, DB, CH>,
    insert_futures: FuturesUnordered<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>,
    current_block:  u64,
    end_block:      u64,
    libmdbx:        &'static DB,
    inspectors:     &'static [&'static dyn Inspector<Result = P::InspectType>],
    progress_bar:   Option<ProgressBar>,
    global_metrics: GlobalRangeMetrics,
    local_metrics:  RangeMetrics,
    _p:             PhantomData<P>,
}

impl<T: TracingProvider, DB: LibmdbxReader + DBWriter, CH: ClickhouseHandle, P: Processor>
    RangeExecutorWithPricing<T, DB, CH, P>
{
    pub fn new(
        id: usize,
        start_block: u64,
        end_block: u64,
        state_collector: StateCollector<T, DB, CH>,
        libmdbx: &'static DB,
        inspectors: &'static [&'static dyn Inspector<Result = P::InspectType>],
        progress_bar: Option<ProgressBar>,
        global_metrics: GlobalRangeMetrics,
    ) -> Self {
        let local_metrics = RangeMetrics::new(&format!("range {id}"));
        local_metrics
            .total_blocks
            .increment(end_block - start_block);

        Self {
            collector: state_collector,
            insert_futures: FuturesUnordered::default(),
            current_block: start_block,
            end_block,
            libmdbx,
            inspectors,
            progress_bar,
            global_metrics,
            local_metrics,
            _p: PhantomData,
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

        while data_batching.insert_futures.next().await.is_some() {
            data_batching.global_metrics.finished_block();
            data_batching.local_metrics.finished_block();
        }

        drop(graceful_guard);
    }

    fn on_price_finish(&mut self, tree: BlockTree<Action>, meta: Metadata) {
        debug!(target:"brontes","Completed DEX pricing");
        self.insert_futures
            .push(Box::pin(self.global_metrics.clone().meter_processing(|| {
                Box::pin(P::process_results(self.libmdbx, self.inspectors, tree, meta))
            })));
    }
}

impl<T: TracingProvider, DB: LibmdbxReader + DBWriter, CH: ClickhouseHandle, P: Processor> Future
    for RangeExecutorWithPricing<T, DB, CH, P>
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if !self.collector.is_collecting_state()
            && self.collector.should_process_next_block()
            && self.insert_futures.len() < 5
            && self.current_block != self.end_block
        {
            cx.waker().wake_by_ref();
            let block = self.current_block;
            self.collector.fetch_state_for(block);
            self.current_block += 1;
            if let Some(pb) = self.progress_bar.as_ref() {
                pb.inc(1)
            };
        }

        while let Poll::Ready(result) = self.collector.poll_next_unpin(cx) {
            match result {
                Some((tree, meta)) => {
                    self.on_price_finish(tree, meta);
                }
                None if self.insert_futures.is_empty() => return Poll::Ready(()),
                _ => break,
            }
        }

        // poll insertion
        while let Poll::Ready(Some(_)) = self.insert_futures.poll_next_unpin(cx) {
            self.global_metrics.finished_block();
            self.local_metrics.finished_block();
        }

        // mark complete if we are done with the range
        if self.current_block == self.end_block
            && self.insert_futures.is_empty()
            && !self.collector.is_collecting_state()
        {
            cx.waker().wake_by_ref();
            self.collector.range_finished();
        }

        Poll::Pending
    }
}
