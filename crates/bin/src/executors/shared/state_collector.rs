use std::{
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering::SeqCst},
        Arc,
    },
    task::Poll,
};

use brontes_classifier::Classifier;
use brontes_core::{decoding::Parser, LibmdbxReader, LibmdbxWriter};
use brontes_database::clickhouse::Clickhouse;
use brontes_pricing::{types::DexPriceMsg, BrontesBatchPricer, GraphManager};
use brontes_types::{
    db::metadata::MetadataCombined, normalized_actions::Actions, traits::TracingProvider, BlockTree,
};
use eyre::eyre;
use futures::{executor, stream::Buffered, Stream, StreamExt};
use reth_tasks::TaskExecutor;
use tracing::info;

use super::metadata::MetadataFetcher;

type CollectionFut<'a> =
    Pin<Box<dyn Future<Output = eyre::Result<BlockTree<Actions>>> + Send + 'a>>;

pub struct StateCollector<T: TracingProvider, DB: LibmdbxReader + LibmdbxWriter> {
    mark_as_finished: Arc<AtomicBool>,
    metadata_fetcher: MetadataFetcher<T, DB>,
    classifier:       &'static Classifier<'static, T, DB>,
    parser:           &'static Parser<'static, T, DB>,
    db:               &'static DB,

    collection_future: Option<CollectionFut<'static>>,
}

impl<T: TracingProvider, DB: LibmdbxReader + LibmdbxWriter> StateCollector<T, DB> {
    pub fn new(
        mark_as_finished: Arc<AtomicBool>,
        metadata_fetcher: MetadataFetcher<T, DB>,
        classifier: &'static Classifier<'static, T, DB>,
        parser: &'static Parser<'static, T, DB>,
        db: &'static DB,
    ) -> Self {
        Self { mark_as_finished, metadata_fetcher, classifier, parser, db, collection_future: None }
    }

    pub fn is_running_pricing(&self) -> bool {
        self.metadata_fetcher.running_pricing()
    }

    pub fn get_shutdown(&self) -> Arc<AtomicBool> {
        self.mark_as_finished.clone()
    }

    pub fn get_price_channel(&mut self) -> Option<UnboundedReceiver<DexPriceMsg>> {
        self.metadata_fetcher.get_price_channel()
    }

    pub fn is_collecting_state(&self) -> bool {
        self.collection_future.is_some()
    }

    pub fn into_tip_mode(
        &mut self,
        pricer: BrontesBatchPricer<T, DB>,
        clickhouse: &'static Clickhouse,
        executor: TaskExecutor,
    ) {
        self.metadata_fetcher
            .into_tip_mode(pricer, clickhouse, executor)
    }

    pub fn fetch_state_for(&mut self, block: u64) {
        self.collection_future = Some(Box::pin(async move {
            let (traces, header) = parser
                .execute(block)
                .await?
                .ok_or_else(|| eyre!("no traces for block {block}"))?;

            info!("Got {} traces + header", traces.len());
            Ok(classifier.build_block_tree(traces, header).await)
        }));
    }

    pub fn range_finished(&self) {
        self.mark_as_finished.store(true, SeqCst);
    }
}

impl<T: TracingProvider, DB: LibmdbxReader + LibmdbxWriter> Stream for StateCollector<T, DB> {
    type Item = (BlockTree<Actions>, MetadataCombined);

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        if let Some(collection_future) = self.collection_future.take() {
            match collection_future.poll_unpin() {
                Poll::Ready(Ok(tree)) => {
                    self.metadata_fetcher.load_metadata_for_tree(tree, self.db);
                }
                Poll::Ready(Err(e)) => {
                    tracing::error!(error = e, "state collector");
                    return Poll::Ready(None)
                }
                Poll::Pending => {
                    self.collection_future = Some(collection_future);
                }
            }
        }

        self.metadata_fetcher.poll_next_unpin(cx)
    }
}
