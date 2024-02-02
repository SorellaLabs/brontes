use std::{
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering::SeqCst},
        Arc,
    },
    task::Poll,
};

use brontes_classifier::Classifier;
use brontes_core::decoding::Parser;
use brontes_database::clickhouse::Clickhouse;
use brontes_pricing::{types::DexPriceMsg, BrontesBatchPricer};
use brontes_types::{
    db::{
        metadata::MetadataCombined,
        traits::{LibmdbxReader, LibmdbxWriter},
    },
    normalized_actions::Actions,
    traits::TracingProvider,
    BlockTree,
};
use eyre::eyre;
use futures::{Future, FutureExt, Stream, StreamExt};
use reth_tasks::TaskExecutor;
use tokio::sync::mpsc::UnboundedReceiver;
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


    pub fn get_shutdown(&self) -> Arc<AtomicBool> {
        self.mark_as_finished.clone()
    }


    pub fn is_collecting_state(&self) -> bool {
        self.collection_future.is_some()
    }


    pub fn fetch_state_for(&mut self, block: u64) {
        let execute_fut = self.parser.execute(block);
        self.collection_future = Some(Box::pin(async {
            let (traces, header) = execute_fut.await?.ok_or_else(|| eyre!("no traces found"))?;

            info!("Got {} traces + header", traces.len());
            Ok(self.classifier.build_block_tree(traces, header).await)
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
        if let Some(mut collection_future) = self.collection_future.take() {
            match collection_future.poll_unpin(cx) {
                Poll::Ready(Ok(tree)) => {
                    let db = self.db;
                    self.metadata_fetcher.load_metadata_for_tree(tree, db);
                }
                Poll::Ready(Err(e)) => {
                    tracing::error!(error = %e, "state collector");
                    return Poll::Ready(None)
                }
                Poll::Pending => {
                    self.collection_future = Some(collection_future);
                }
            }
        }

        if self.metadata_fetcher.is_finished() {
            return Poll::Ready(None)
        }

        self.metadata_fetcher.poll_next_unpin(cx)
    }
}
