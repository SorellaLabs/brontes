use std::{
    pin::Pin,
    task::{Context, Poll},
};

use alloy_primitives::Address;
use brontes_core::decoding::{Parser, TracingProvider};
use brontes_database::{
    clickhouse::ClickhouseHandle,
    libmdbx::{DBWriter, LibmdbxReader},
};
use brontes_inspect::Inspector;
use brontes_types::{
    db::metadata::Metadata, mev::Bundle, normalized_actions::Actions, tree::BlockTree,
};
use futures::{pin_mut, stream::FuturesUnordered, Future, StreamExt};
use reth_tasks::shutdown::GracefulShutdown;
use tracing::{debug, info};

use super::shared::{inserts::process_results, state_collector::StateCollector};

pub struct TipInspector<T: TracingProvider, DB: LibmdbxReader + DBWriter, CH: ClickhouseHandle> {
    current_block: u64,
    parser: &'static Parser<'static, T, DB>,
    state_collector: StateCollector<T, DB, CH>,
    database: &'static DB,
    inspectors: &'static [&'static dyn Inspector<Result = Vec<Bundle>>],
    processing_futures: FuturesUnordered<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>,
}

impl<T: TracingProvider, DB: DBWriter + LibmdbxReader, CH: ClickhouseHandle>
    TipInspector<T, DB, CH>
{
    pub fn new(
        current_block: u64,
        _quote_asset: Address,
        state_collector: StateCollector<T, DB, CH>,
        parser: &'static Parser<'static, T, DB>,
        database: &'static DB,
        inspectors: &'static [&'static dyn Inspector<Result = Vec<Bundle>>],
    ) -> Self {
        Self {
            state_collector,
            inspectors,
            current_block,
            parser,
            processing_futures: FuturesUnordered::new(),
            database,
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

        while (tip.processing_futures.next().await).is_some() {}

        drop(graceful_guard);
    }

    #[cfg(feature = "local-reth")]
    fn start_block_inspector(&mut self) -> bool {
        if self.state_collector.is_collecting_state() {
            return false;
        }

        match self.parser.get_latest_block_number() {
            Ok(chain_tip) => {
                if chain_tip > self.current_block {
                    self.current_block += 1;
                    true
                } else {
                    false
                }
            }
            Err(e) => {
                debug!("Error: {:?}", e);
                false
            }
        }
    }

    #[cfg(not(feature = "local-reth"))]
    fn start_block_inspector(&mut self) -> bool {
        if self.state_collector.is_collecting_state() {
            return false;
        }

        let cur_block = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { self.parser.get_latest_block_number().await })
        });

        match cur_block {
            Ok(chain_tip) => {
                if chain_tip > self.current_block {
                    self.current_block += 1;
                    true
                } else {
                    false
                }
            }
            Err(e) => {
                debug!("Error: {:?}", e);
                false
            }
        }
    }

    fn on_price_finish(&mut self, tree: BlockTree<Actions>, meta: Metadata) {
        info!(target:"brontes","Completed DEX pricing");
        self.processing_futures.push(Box::pin(process_results(
            self.database,
            self.inspectors,
            tree.into(),
            meta.into(),
        )));
    }
}

impl<T: TracingProvider, DB: DBWriter + LibmdbxReader, CH: ClickhouseHandle> Future
    for TipInspector<T, DB, CH>
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.start_block_inspector() {
            let block = self.current_block;
            self.state_collector.fetch_state_for(block);
        }
        if let Poll::Ready(item) = self.state_collector.poll_next_unpin(cx) {
            match item {
                Some((tree, meta)) => self.on_price_finish(tree, meta),
                None => return Poll::Ready(()),
            }
        }
        while let Poll::Ready(Some(_)) = self.processing_futures.poll_next_unpin(cx) {}

        Poll::Pending
    }
}
