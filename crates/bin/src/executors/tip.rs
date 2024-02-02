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
use brontes_types::{db::metadata::MetadataNoDex, normalized_actions::Actions, tree::BlockTree};
use futures::{stream::FuturesUnordered, Future, FutureExt, StreamExt};
use reth_tasks::TaskExecutor;
use tokio::sync::{futures, mpsc::UnboundedReceiver};
use tracing::{debug, info};

use super::shared::{
    inserts::process_results,
    metadata::MetadataFetcher,
    state_collector::{collect_all_state, StateCollector},
};

pub struct TipInspector<T: TracingProvider, DB: LibmdbxReader + LibmdbxWriter> {
    current_block:      u64,
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
        database: &'static DB,
        clickhouse: &'static Clickhouse,
        inspectors: &'static [&'static Box<dyn Inspector>],
        task_executor: TaskExecutor,
    ) -> Self {
        // put into pricing mode if not already
        if !state_collector.is_running_pricing() {
            let pairs = database.protocols_created_before(current_block).unwrap();
            let pair_graph = GraphManager::init_from_db_state(pairs, HashMap::default(), database);

            let price_chan = state_collector.get_price_channel();

            let pricer = BrontesBatchPricer::new(
                state_collector.get_shutdown(),
                quote_asset,
                pair_graph,
                price_chan,
                parser.get_tracer(),
                current_block,
                HashMap::new(),
            );
            state_collector.into_tip_mode(pricer, clickhouse, task_executor);
        }

        Self {
            state_collector,
            inspectors,
            current_block,
            processing_futures: FuturesUnordered::new(),
            database,
        }
    }

    fn start_collection(&mut self) {
        info!(block_number = self.current_block, "starting data collection");
        self.processing_future = Some(Box::pin(
            collect_all_state(
                self.current_block,
                self.database,
                self.metadata_fetcher.take().unwrap(),
                self.parser,
                self.classifier,
            )
            .map(|res| async move {
                let (fetcher, tree, metadata) = res?;
                process_results(self.database, self.inspectors, tree, metadata).await;
                fetcher
            }),
        ));
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
}

impl<T: TracingProvider, DB: LibmdbxWriter + LibmdbxReader> Future for TipInspector<T, DB> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        #[cfg(not(feature = "local"))]
        {
            if self.start_block_inspector() {
                self.state_collector.fetch_state_for(self.current_block);
            }
            if let Some(mut future) = self.processing_future.take() {
                if let Poll::Ready(res) = future.poll_unpin(cx) {
                    match res {
                        Ok(fetcher) => self.metadata_fetcher = Some(fetcher),
                        Err(e) => {
                            tracing::error!(error = e, "tip inspector ran into a error");
                            return Poll::Ready(())
                        }
                    }
                } else {
                    self.processing_future = Some(future);
                }
            }
        }

        Poll::Pending
    }
}
