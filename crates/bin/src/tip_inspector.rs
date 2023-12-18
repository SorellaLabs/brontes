use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use brontes_classifier::Classifier;
use brontes_core::decoding::{Parser, TracingProvider};
use brontes_database::{clickhouse::Clickhouse, MetadataDB};
use brontes_database_libmdbx::Libmdbx;
use brontes_inspect::{composer::Composer, Inspector};
use brontes_pricing::types::DexPrices;
use brontes_types::{
    classified_mev::{ClassifiedMev, MevBlock, SpecificMev},
    normalized_actions::Actions,
    tree::TimeTree,
};
use futures::{stream::FuturesOrdered, Future, FutureExt, StreamExt};
use tracing::{debug, info};
type CollectionFut<'a> = Pin<Box<dyn Future<Output = (MetadataDB, TimeTree<Actions>)> + Send + 'a>>;

pub struct TipInspector<'inspector, const N: usize, T: TracingProvider> {
    current_block: u64,

    parser:            &'inspector Parser<'inspector, T>,
    classifier:        &'inspector Classifier<'inspector>,
    clickhouse:        &'inspector Clickhouse,
    database:          &'inspector Libmdbx,
    composer:          Composer<'inspector, N>,
    // pending future data
    classifier_future: FuturesOrdered<CollectionFut<'inspector>>,
    // pending insertion data
    insertion_future:  Option<Pin<Box<dyn Future<Output = ()> + Send + Sync + 'inspector>>>,
}

impl<'inspector, const N: usize, T: TracingProvider> TipInspector<'inspector, N, T> {
    pub fn new(
        parser: &'inspector Parser<'inspector, T>,
        clickhouse: &'inspector Clickhouse,
        database: &'inspector Libmdbx,
        classifier: &'inspector Classifier,
        inspectors: &'inspector [&'inspector Box<dyn Inspector>; N],
        current_block: u64,
    ) -> Self {
        Self {
            current_block,
            parser,
            clickhouse,
            database,
            classifier,
            composer: Composer::new(inspectors),
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
            let (_extra_data, mut tree) = self.classifier.build_tree(traces, header);

            let meta = labeller_fut.await;
            tree.eth_price = meta.eth_prices.clone();

            (meta, tree)
        });

        self.classifier_future.push_back(classifier_fut);
    }

    fn on_inspectors_finish(
        &mut self,
        results: (MevBlock, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>),
    ) {
        info!(
            block_number = self.current_block,
            "inserting the collected results \n {:#?}", results
        );
        self.insertion_future =
            Some(Box::pin(self.database.insert_classified_data(results.0, results.1)));
    }

    fn progress_futures(&mut self, cx: &mut Context<'_>) {
        match self.classifier_future.poll_next_unpin(cx) {
            Poll::Ready(Some((meta_data, tree))) => {
                //TODO: wire in the dex pricing task here
                let meta_data = Arc::new(meta_data.into_finalized_metadata(DexPrices::new()));
                self.composer.on_new_tree(tree.into(), meta_data);
            }
            Poll::Pending => return,
            Poll::Ready(None) => return,
        }

        if !self.composer.is_finished() {
            if let Poll::Ready(data) = self.composer.poll_unpin(cx) {
                self.on_inspectors_finish(data);
            }
        }

        if let Some(mut insertion_future) = self.insertion_future.take() {
            match insertion_future.poll_unpin(cx) {
                Poll::Ready(_) => {}
                Poll::Pending => {
                    self.insertion_future = Some(insertion_future);
                }
            }
        }
    }

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
}

impl<const N: usize, T: TracingProvider> Future for TipInspector<'_, N, T> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // If the classifier_future is None (not started yet), start the collection
        // phase

        loop {
            if self.start_block_inspector() {
                self.start_collection();
            }
            self.progress_futures(cx);
        }

        #[allow(unreachable_code)]
        Poll::Pending
    }
}
