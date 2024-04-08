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
use brontes_types::{
    db::metadata::Metadata, mev::events::Action, normalized_actions::Actions, tree::BlockTree,
};
use futures::{pin_mut, stream::FuturesUnordered, Future, StreamExt};
use reth_tasks::shutdown::GracefulShutdown;
//tui related
use tokio::sync::mpsc::UnboundedSender;
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
    //progress_bar:   Option<ProgressBar>,
    tui_tx:         UnboundedSender<Action>,
    _p:             PhantomData<P>,
}

impl<T: TracingProvider, DB: LibmdbxReader + DBWriter, CH: ClickhouseHandle, P: Processor>
    RangeExecutorWithPricing<T, DB, CH, P>
{
    pub fn new(
        start_block: u64,
        end_block: u64,
        state_collector: StateCollector<T, DB, CH>,
        libmdbx: &'static DB,
        inspectors: &'static [&'static dyn Inspector<Result = P::InspectType>],
        tui_tx: Option<UnboundedSender<Action>>,
        //progress_bar: Option<ProgressBar>,
    ) -> Self {
        Self {
            collector: state_collector,
            insert_futures: FuturesUnordered::default(),
            current_block: start_block,
            end_block,
            libmdbx,
            inspectors,
            tui_tx,
            //progress_bar,
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

        drop(graceful_guard);
    }

    fn on_price_finish(&mut self, tree: BlockTree<Actions>, meta: Metadata) {
        debug!(target:"brontes","Completed DEX pricing");
        self.insert_futures.push(Box::pin(P::process_results(
            self.libmdbx,
            self.inspectors,
            tree.into(),
            meta.into(),
            self.tui_tx.clone(),
        )));
    }
}

impl<T: TracingProvider, DB: LibmdbxReader + DBWriter, CH: ClickhouseHandle, P: Processor> Future
    for RangeExecutorWithPricing<T, DB, CH, P>
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut work = 256;
        loop {
            if !self.collector.is_collecting_state()
                && self.collector.should_process_next_block()
                && self.current_block != self.end_block
            {
                let block = self.current_block;
                self.collector.fetch_state_for(block);
                self.current_block += 1;
                /*
                if let Some(pb) = self.progress_bar.as_ref() {
                    pb.inc(1)
                };
                */
            }

            if let Poll::Ready(result) = self.collector.poll_next_unpin(cx) {
                match result {
                    Some((tree, meta)) => {
                        self.on_price_finish(tree, meta);
                    }
                    None if self.insert_futures.is_empty() => return Poll::Ready(()),
                    _ => {}
                }
            }

            // poll insertion
            while let Poll::Ready(Some(_)) = self.insert_futures.poll_next_unpin(cx) {}

            // mark complete if we are done with the range
            if self.current_block == self.end_block
                && self.insert_futures.is_empty()
                && !self.collector.is_collecting_state()
            {
                self.collector.range_finished();
            }

            work -= 1;
            if work == 0 {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
        }
    }
}
