use std::{
    collections::HashMap,
    fs::File,
    io::Write,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering::SeqCst},
        Arc,
    },
    task::{Context, Poll},
};

use alloy_primitives::Address;
use brontes_classifier::Classifier;
use brontes_core::decoding::{Parser, TracingProvider};
use brontes_database::{
    clickhouse::Clickhouse,
    libmdbx::{LibmdbxReader, LibmdbxWriter},
};
use brontes_inspect::Inspector;
use brontes_pricing::{types::DexPriceMsg, BrontesBatchPricer, GraphManager};
use brontes_types::{
    constants::START_OF_CHAINBOUND_MEMPOOL_DATA,
    db::metadata::{MetadataCombined, MetadataNoDex},
    mev::PossibleMevCollection,
    normalized_actions::Actions,
    structured_trace::TxTrace,
    tree::BlockTree,
};
use futures::{pin_mut, stream::FuturesUnordered, Future, FutureExt, StreamExt};
use reth_primitives::Header;
use reth_tasks::{shutdown::GracefulShutdown, TaskExecutor};
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::{debug, error, info};

use super::shared::{
    inserts::process_results,
    metadata::MetadataFetcher,
    state_collector::{self, StateCollector},
};
use crate::TipInspector;

type CollectionFut<'a> =
    Pin<Box<dyn Future<Output = (BlockTree<Actions>, MetadataNoDex)> + Send + 'a>>;

pub struct RangeExecutorWithPricing<T: TracingProvider + Clone, DB: LibmdbxWriter + LibmdbxReader> {
    collector:      StateCollector<T, DB>,
    insert_futures: FuturesUnordered<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>,

    current_block: u64,
    end_block:     u64,
    batch_id:      u64,

    libmdbx:    &'static DB,
    inspectors: &'static [&'static Box<dyn Inspector>],
}

impl<T: TracingProvider + Clone, DB: LibmdbxReader + LibmdbxWriter>
    RangeExecutorWithPricing<T, DB>
{
    pub fn new(
        quote_asset: Address,
        batch_id: u64,
        start_block: u64,
        end_block: u64,
        state_collector: StateCollector<T, DB>,
        libmdbx: &'static DB,
        inspectors: &'static [&'static Box<dyn Inspector>],
    ) -> Self {
        Self {
            collector: state_collector,
            insert_futures: FuturesUnordered::default(),
            current_block: start_block,
            end_block,
            batch_id,
            libmdbx,
            inspectors,
        }
    }

    pub async fn run_until_graceful_shutdown(self, shutdown: GracefulShutdown) {
        let data_batching = self;
        pin_mut!(data_batching, shutdown);

        let mut graceful_guard = None;
        tokio::select! {
            _= &mut data_batching => {

            },
            guard = shutdown => {
                graceful_guard = Some(guard);
            },
        }

        drop(graceful_guard);
    }

    fn on_price_finish(&mut self, tree: BlockTree<Actions>, meta: MetadataCombined) {
        info!(target:"brontes","dex pricing finished");
        self.insert_futures.push(Box::pin(process_results(
            self.libmdbx,
            self.inspectors,
            tree.into(),
            meta.into(),
        )));
    }
}

impl<T: TracingProvider + Clone, DB: LibmdbxReader + LibmdbxWriter> Future
    for RangeExecutorWithPricing<T, DB>
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut work = 256;
        loop {
            if !self.collector.is_collecting_state() && self.current_block != self.end_block {
                let block = self.current_block;
                self.collector.fetch_state_for(block);
                self.current_block += 1;
            }

            if let Poll::Ready(result) = self.collector.poll_next_unpin(cx) {
                match result {
                    Some((tree, meta)) => {
                        self.on_price_finish(tree, meta);
                    }
                    None => return Poll::Ready(()),
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
                return Poll::Pending
            }
        }
    }
}
