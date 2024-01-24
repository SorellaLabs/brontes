use std::{
    collections::HashMap,
    future::IntoFuture,
    pin::{pin, Pin},
    sync::Arc,
    task::{Context, Poll},
};

use brontes_classifier::Classifier;
use brontes_core::{
    decoding::{Parser, TracingProvider},
    missing_decimals::load_missing_decimals,
};
use brontes_database::{
    clickhouse::Clickhouse,
    libmdbx::{
        tables::MevBlocks, types::mev_block::MevBlocksData, Libmdbx, LibmdbxReader, LibmdbxWriter,
    },
};
use brontes_inspect::{
    composer::{compose_mev_results, ComposerResults},
    Inspector,
};
use brontes_types::{
    classified_mev::{BundleData, BundleHeader, MevBlock},
    db::{dex::DexQuotes, metadata::MetadataNoDex, mev_block::MevBlockWithClassified},
    normalized_actions::Actions,
    tree::BlockTree,
};
use futures::{stream::FuturesOrdered, Future, FutureExt, StreamExt};
use tracing::{debug, error, info};

type CollectionFut<'a> =
    Pin<Box<dyn Future<Output = (MetadataNoDex, BlockTree<Actions>)> + Send + 'a>>;

pub struct TipInspector<'inspector, T: TracingProvider, DB: LibmdbxReader + LibmdbxWriter> {
    current_block: u64,

    parser:            &'inspector Parser<'inspector, T, DB>,
    classifier:        &'inspector Classifier<'inspector, T, DB>,
    clickhouse:        &'inspector Clickhouse,
    database:          &'inspector DB,
    inspectors:        &'inspector [&'inspector Box<dyn Inspector>],
    // pending future data
    classifier_future: FuturesOrdered<CollectionFut<'inspector>>,

    composer_future:  Option<Pin<Box<dyn Future<Output = ComposerResults> + Send + 'inspector>>>,
    // pending insertion data
    #[allow(dead_code)]
    insertion_future: Option<Pin<Box<dyn Future<Output = ()> + Send + Sync + 'inspector>>>,
}

impl<'inspector, T: TracingProvider, DB: LibmdbxWriter + LibmdbxReader>
    TipInspector<'inspector, T, DB>
{
    pub fn new(
        parser: &'inspector Parser<'inspector, T, DB>,
        clickhouse: &'inspector Clickhouse,
        database: &'inspector DB,
        classifier: &'inspector Classifier<'_, T, DB>,
        inspectors: &'inspector [&'inspector Box<dyn Inspector>],
        current_block: u64,
    ) -> Self {
        Self {
            inspectors,
            current_block,
            parser,
            clickhouse,
            composer_future: None,
            database,
            classifier,
            classifier_future: FuturesOrdered::new(),
            insertion_future: None,
        }
    }

    fn start_collection(&mut self) {
        info!(block_number = self.current_block, "starting data collection");
        let parser_fut = self.parser.execute(self.current_block);
        let labeller_fut = self.clickhouse.get_metadata(self.current_block);

        let classifier_fut = Box::pin(async {
            let (traces, header) = parser_fut.await.unwrap().unwrap();
            info!("Got {} traces + header", traces.len());
            let block = header.number;
            let (extra_data, tree) = self.classifier.build_block_tree(traces, header).await;

            load_missing_decimals(
                self.parser.get_tracer(),
                self.database,
                block,
                extra_data.tokens_decimal_fill,
            )
            .await;

            let meta = labeller_fut.await;

            (meta, tree)
        });

        self.classifier_future.push_back(classifier_fut);
    }

    fn on_inspectors_finish(&mut self, results: ComposerResults) {
        info!(
            block_number = self.current_block,
            "inserting the collected results \n {:#?}", results
        );

        if self
            .database
            .save_mev_blocks(self.current_block, results.block_details, results.mev_details)
            .is_err()
        {
            error!("failed to insert classified data into libmdx");
        }
    }

    fn progress_futures(&mut self, cx: &mut Context<'_>) {
        match self.classifier_future.poll_next_unpin(cx) {
            Poll::Ready(Some((meta_data, tree))) => {
                let meta_data = meta_data.into_finalized_metadata(DexQuotes(vec![]));
                //TODO: wire in the dex pricing task here

                self.composer_future = Some(Box::pin(compose_mev_results(
                    self.inspectors,
                    tree.into(),
                    meta_data.into(),
                )));
            }
            Poll::Pending => return,
            Poll::Ready(None) => return,
        }

        if let Some(mut inner) = self.composer_future.take() {
            if let Poll::Ready(data) = inner.poll_unpin(cx) {
                self.on_inspectors_finish(data);
            } else {
                self.composer_future = Some(inner);
            }
        }
    }

    #[cfg(not(feature = "local"))]
    fn start_block_inspector(&mut self) -> bool {
        match self.parser.get_latest_block_number() {
            Ok(chain_tip) => {
                if chain_tip > self.current_block {
                    self.current_block = chain_tip;
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
                    self.current_block = chain_tip;
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

impl<T: TracingProvider, DB: LibmdbxWriter + LibmdbxReader> Future for TipInspector<'_, T, DB> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // If the classifier_future is None (not started yet), start the collection
        // phase

        loop {
            #[cfg(not(feature = "local"))]
            {
                if self.start_block_inspector() {
                    self.start_collection();
                }
                self.progress_futures(cx);
            }
        }

        #[allow(unreachable_code)]
        Poll::Pending
    }
}
