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
use brontes_metrics::range::GlobalRangeMetrics;
use brontes_types::MultiBlockData;
use futures::{pin_mut, stream::FuturesUnordered, Future, StreamExt};
use reth_tasks::shutdown::GracefulShutdown;
use tracing::debug;

use super::shared::state_collector::StateCollector;
use crate::{executors::ProgressBar, Processor};

type InsertFutures = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

pub struct RangeExecutorWithPricing<
    T: TracingProvider,
    DB: DBWriter + LibmdbxReader,
    CH: ClickhouseHandle,
    P: Processor,
> {
    id:             usize,
    collector:      StateCollector<T, DB, CH>,
    insert_futures: FuturesUnordered<InsertFutures>,
    current_block:  u64,
    end_block:      u64,
    libmdbx:        &'static DB,
    inspectors:     &'static [&'static dyn Inspector<Result = P::InspectType>],
    progress_bar:   Option<ProgressBar>,
    global_metrics: Option<GlobalRangeMetrics>,
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
        global_metrics: Option<GlobalRangeMetrics>,
    ) -> Self {
        Self {
            id,
            collector: state_collector,
            insert_futures: FuturesUnordered::default(),
            current_block: start_block,
            end_block,
            libmdbx,
            inspectors,
            progress_bar,
            global_metrics,
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
            data_batching
                .global_metrics
                .as_ref()
                .inspect(|m| m.finished_block(data_batching.id));
        }

        drop(graceful_guard);
    }

    fn on_price_finish(&mut self, data: MultiBlockData) {
        debug!(target:"brontes","Completed DEX pricing");
        self.global_metrics
            .as_ref()
            .inspect(|m| m.inc_inspector(self.id));

        let metrics = self.global_metrics.clone();
        let inspectors = self.inspectors;
        let libmdbx = self.libmdbx;
        self.insert_futures.push(Box::pin(async move {
            if let Some(metrics) = metrics {
                metrics
                    .meter_processing(|| Box::pin(P::process_results(libmdbx, inspectors, data)))
                    .await
            } else {
                P::process_results(libmdbx, inspectors, data).await
            }
        }));
    }
}

impl<T: TracingProvider, DB: LibmdbxReader + DBWriter, CH: ClickhouseHandle, P: Processor> Future
    for RangeExecutorWithPricing<T, DB, CH, P>
{
    type Output = ();

    #[brontes_macros::metrics_call(ptr=global_metrics, poll_rate, self.id)]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if !self.collector.is_collecting_state()
            && self.collector.should_process_next_block()
            && self.current_block != self.end_block
            && self.insert_futures.len() < 5
        {
            cx.waker().wake_by_ref();
            let block = self.current_block;
            let id = self.id;
            let metrics = self.global_metrics.clone();
            self.collector.fetch_state_for(block, id, metrics);

            self.current_block += 1;
            if let Some(pb) = self.progress_bar.as_ref() {
                pb.inc(1)
            };
        }

        while let Poll::Ready(result) = self.collector.poll_next_unpin(cx) {
            match result {
                Some(data) => {
                    self.global_metrics
                        .as_ref()
                        .inspect(|m| m.remove_pending_tree(self.id));
                    self.on_price_finish(data);
                }
                None if self.insert_futures.is_empty() && self.current_block == self.end_block => {
                    return Poll::Ready(())
                }
                None => {
                    cx.waker().wake_by_ref();
                    break;
                }
            }
        }

        while let Poll::Ready(Some(_)) = self.insert_futures.poll_next_unpin(cx) {
            self.global_metrics.as_ref().inspect(|m| {
                m.dec_inspector(self.id);
                m.finished_block(self.id);
            });
        }

        // mark complete if we are done with the range
        if self.current_block == self.end_block
            && self.insert_futures.is_empty()
            && !self.collector.is_collecting_state()
        {
            self.collector.range_finished(cx.waker());
        }

        Poll::Pending
    }
}
