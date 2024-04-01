use std::{
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use brontes_core::decoding::{Parser, TracingProvider};
use brontes_database::{
    clickhouse::ClickhouseHandle,
    libmdbx::{DBWriter, LibmdbxReader},
};
use brontes_inspect::Inspector;
use brontes_types::{db::metadata::Metadata, normalized_actions::Actions, tree::BlockTree};
use futures::{pin_mut, stream::FuturesUnordered, Future, StreamExt};
use reth_tasks::shutdown::GracefulShutdown;
use tracing::debug;

use super::shared::state_collector::StateCollector;
use crate::Processor;

pub struct TipInspector<
    T: TracingProvider,
    DB: LibmdbxReader + DBWriter,
    CH: ClickhouseHandle,
    P: Processor,
> {
    current_block:      u64,
    back_from_tip:      u64,
    parser:             &'static Parser<'static, T, DB>,
    state_collector:    StateCollector<T, DB, CH>,
    database:           &'static DB,
    inspectors:         &'static [&'static dyn Inspector<Result = P::InspectType>],
    processing_futures: FuturesUnordered<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>,
    _p:                 PhantomData<P>,
}

impl<T: TracingProvider, DB: DBWriter + LibmdbxReader, CH: ClickhouseHandle, P: Processor>
    TipInspector<T, DB, CH, P>
{
    pub fn new(
        current_block: u64,
        back_from_tip: u64,
        state_collector: StateCollector<T, DB, CH>,
        parser: &'static Parser<'static, T, DB>,
        database: &'static DB,
        inspectors: &'static [&'static dyn Inspector<Result = P::InspectType>],
    ) -> Self {
        Self {
            back_from_tip,
            state_collector,
            inspectors,
            current_block,
            parser,
            processing_futures: FuturesUnordered::new(),
            database,
            _p: PhantomData,
        }
    }

    pub async fn run_until_graceful_shutdown(self, shutdown: GracefulShutdown) {
        let tip = self;
        pin_mut!(tip, shutdown);

        let mut graceful_guard = None;
        tokio::select! {
            _= &mut tip => {
            },
            guard = shutdown => {
                graceful_guard = Some(guard);
            },
        }

        drop(graceful_guard);
    }

    #[cfg(feature = "local-reth")]
    fn start_block_inspector(&mut self) -> bool {
        if self.state_collector.is_collecting_state() {
            return false
        }

        match self.parser.get_latest_block_number() {
            Ok(chain_tip) => chain_tip - self.back_from_tip > self.current_block,
            Err(e) => {
                tracing::error!("Error: {:?}", e);
                false
            }
        }
    }

    #[cfg(not(feature = "local-reth"))]
    fn start_block_inspector(&mut self) -> bool {
        if self.state_collector.is_collecting_state() {
            return false
        }

        let cur_block = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { self.parser.get_latest_block_number().await })
        });

        match cur_block {
            Ok(chain_tip) => chain_tip - self.back_from_tip > self.current_block,
            Err(e) => {
                tracing::error!("Error: {:?}", e);
                false
            }
        }
    }

    fn on_price_finish(&mut self, tree: BlockTree<Actions>, meta: Metadata) {
        debug!(target:"brontes::tip_inspector","Completed DEX pricing");
        self.processing_futures.push(Box::pin(P::process_results(
            self.database,
            self.inspectors,
            tree.into(),
            meta.into(),
        )));
    }
}

impl<T: TracingProvider, DB: DBWriter + LibmdbxReader, CH: ClickhouseHandle, P: Processor> Future
    for TipInspector<T, DB, CH, P>
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.start_block_inspector() && self.state_collector.should_process_next_block() {
            tracing::info!("starting new tip block");
            let block = self.current_block;
            self.state_collector.fetch_state_for(block);
            self.current_block += 1;
        }

        if let Poll::Ready(item) = self.state_collector.poll_next_unpin(cx) {
            match item {
                Some((tree, meta)) => self.on_price_finish(tree, meta),
                None if self.processing_futures.is_empty() => return Poll::Ready(()),
                _ => {}
            }
        }
        while let Poll::Ready(Some(_)) = self.processing_futures.poll_next_unpin(cx) {}

        cx.waker().wake_by_ref();
        Poll::Pending
    }
}
