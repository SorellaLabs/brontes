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
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::{debug, info};

use super::{
    dex_pricing::WaitingForPricerFuture, shared::metadata::MetadataFetcher, utils::process_results,
};


pub struct TipInspector<T: TracingProvider, DB: LibmdbxReader + LibmdbxWriter> {
    current_block: u64,
    dex_pricing_shutdown_signal: Arc<AtomicBool>,
    metadata_fetcher: Option<MetadataFetcher<T, DB>>,
    parser: &'static Parser<'static, T, DB>,
    classifier: &'static Classifier<'static, T, DB>,
    database: &'static DB,
    inspectors: &'static [&'static Box<dyn Inspector>],
    processing_future: Option<Pin<Box<dyn Future<Output = ()> +Send + 'static>>,
}

impl<T: TracingProvider, DB: LibmdbxWriter + LibmdbxReader> TipInspector<T, DB> {
    pub fn new(
        parser: &'static Parser<'static, T, DB>,
        database: &'static DB,
        classifier: &'static Classifier<'_, T, DB>,
        inspectors: &'static [&'static Box<dyn Inspector>],
        fetcher: MetadataFetcher<T, DB>,
        current_block: u64,
        task_executor: TaskExecutor,
        rx: UnboundedReceiver<DexPriceMsg>,
        quote_asset: Address,
    ) -> Self {
        let pairs = database.protocols_created_before(current_block).unwrap();

        let pair_graph = GraphManager::init_from_db_state(pairs, HashMap::default(), database);

        let pricer = BrontesBatchPricer::new(
            Arc::new(AtomicBool::new(false)),
            quote_asset,
            pair_graph,
            rx,
            parser.get_tracer(),
            current_block,
            HashMap::new(),
        );
        Self {
            inspectors,
            current_block,
            parser,
            composer_future: FuturesUnordered::new(),
            database,
            classifier,
            classifier_future: None,
        }
    }

    pub async fn shutdown(mut self) {
        if let Some(fut) = self.classifier_future.take() {
            let (meta, tree) = fut.await;
            self.pricer
                .add_pending_inspection(self.current_block, tree, meta);
        }
        self.classifier.close();

        // triggers pricing shutdown
        while let Some((tree, meta_data)) = self.pricer.next().await {
            self.classifier.close();
            self.composer_future.push(Box::pin(
                process_results(self.database, self.inspectors, tree.into(), meta_data.into())
                    .map(|_| ()),
            ));
        }

        while let Some(_) = self.composer_future.next().await {}
        info!("tip inspector properly shutdown");
    }

    fn start_collection(&mut self) {
        info!(block_number = self.current_block, "starting data collection");
        let parser_fut = self.parser.execute(self.current_block);
        let labeller_fut = self.clickhouse.get_metadata(self.current_block);

        let classifier_fut = Box::pin(async {
            let (traces, header) = parser_fut.await.unwrap().unwrap();
            info!("Got {} traces + header", traces.len());
            let tree = self.classifier.build_block_tree(traces, header).await;
            let meta = labeller_fut.await;

            (meta, tree)
        });

        self.classifier_future = Some(classifier_fut);
    }

    fn progress_futures(&mut self, cx: &mut Context<'_>) {
        if let Some(mut future) = self.classifier_future.take() {
            if let Poll::Ready((meta, tree)) = future.poll_unpin(cx) {
                debug!("built tree");
                let block = self.current_block;
                self.pricer.add_pending_inspection(block, tree, meta);
            } else {
                self.classifier_future = Some(future);
            }
        }
        if let Poll::Ready(Some(_)) = self.composer_future.poll_next_unpin(cx) {}
    }

    #[cfg(not(feature = "local"))]
    fn start_block_inspector(&mut self) -> bool {
        if self.classifier_future.is_some() {
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
                self.start_collection();
            }
            self.progress_futures(cx);
            if let Poll::Ready(Some((tree, meta))) = self.pricer.poll_next_unpin(cx) {
                self.composer_future.push(Box::pin(
                    process_results(self.database, self.inspectors, tree.into(), meta.into())
                        .map(|_| ()),
                ));
            }
        }

        Poll::Pending
    }
}
