use std::{
    collections::HashMap,
    pin::Pin,
    sync::{atomic::AtomicBool, Arc},
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
    db::metadata::{MetadataCombined, MetadataNoDex},
    normalized_actions::Actions,
    tree::BlockTree,
};
use futures::{pin_mut, stream::FuturesUnordered, Future, FutureExt, StreamExt};
use reth_tasks::{shutdown::GracefulShutdown, TaskExecutor};
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::{debug, info};

use super::shared::{
    inserts::process_results, metadata::MetadataFetcher, state_collector::StateCollector,
};

pub struct TipInspector<T: TracingProvider, DB: LibmdbxReader + LibmdbxWriter> {
    current_block:      u64,
    parser:             &'static Parser<'static, T, DB>,
    state_collector:    StateCollector<T, DB>,
    database:           &'static DB,
    inspectors:         &'static [&'static Box<dyn Inspector>],
    processing_futures: FuturesUnordered<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>,
}

impl<T: TracingProvider, DB: LibmdbxWriter + LibmdbxReader> TipInspector<T, DB> {
    pub fn new(
        current_block: u64,
        quote_asset: Address,
        mut state_collector: StateCollector<T, DB>,
        parser: &'static Parser<'static, T, DB>,
        database: &'static DB,
        inspectors: &'static [&'static Box<dyn Inspector>],
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

        while let Some(_) = tip.processing_futures.next().await {}

        drop(graceful_guard);
    }

    #[cfg(not(feature = "local"))]
    fn start_block_inspector(&mut self) -> bool {
        if self.state_collector.is_collecting_state() {
            return false
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

    #[cfg(feature = "local")]
    async fn start_block_inspector(&mut self) -> bool {
        if self.state_collector.is_collecting_state() {
            return false
        }

        match self.parser.get_latest_block_number().await {
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

    fn on_price_finish(&mut self, tree: BlockTree<Actions>, meta: MetadataCombined) {
        info!(target:"brontes","dex pricing finished");
        self.processing_futures.push(Box::pin(process_results(
            self.database,
            self.inspectors,
            tree.into(),
            meta.into(),
        )));
    }
}

impl<T: TracingProvider, DB: LibmdbxWriter + LibmdbxReader> Future for TipInspector<T, DB> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        #[cfg(not(feature = "local"))]
        {
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
        }

        Poll::Pending
    }
}
