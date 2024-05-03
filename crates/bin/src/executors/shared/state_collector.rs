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
use brontes_database::clickhouse::ClickhouseHandle;
use brontes_types::{
    db::{
        metadata::Metadata,
        traits::{DBWriter, LibmdbxReader},
    },
    normalized_actions::Action,
    structured_trace::TxTrace,
    traits::TracingProvider,
    BlockTree,
};
use eyre::eyre;
use futures::{Future, FutureExt, Stream, StreamExt};
use reth_primitives::Header;
use tokio::task::JoinError;
use tracing::{span, trace, Instrument, Level};

use super::metadata::MetadataFetcher;

type CollectionFut<'a> = Pin<Box<dyn Future<Output = eyre::Result<BlockTree<Action>>> + Send + 'a>>;

type ExecutionFut<'a> =
    Pin<Box<dyn Future<Output = Result<Option<(Vec<TxTrace>, Header)>, JoinError>> + Send + 'a>>;

pub struct StateCollector<T: TracingProvider, DB: LibmdbxReader + DBWriter, CH: ClickhouseHandle> {
    mark_as_finished: Arc<AtomicBool>,
    metadata_fetcher: MetadataFetcher<T, DB, CH>,
    classifier:       &'static Classifier<'static, T, DB>,
    parser:           &'static Parser<T, DB>,
    db:               &'static DB,

    collection_future: Option<CollectionFut<'static>>,
}

impl<T: TracingProvider, DB: LibmdbxReader + DBWriter, CH: ClickhouseHandle>
    StateCollector<T, DB, CH>
{
    pub fn new(
        mark_as_finished: Arc<AtomicBool>,
        metadata_fetcher: MetadataFetcher<T, DB, CH>,
        classifier: &'static Classifier<'static, T, DB>,
        parser: &'static Parser<T, DB>,
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

    pub fn should_process_next_block(&self) -> bool {
        self.metadata_fetcher.should_process_next_block()
    }

    async fn state_future(
        generate_pricing: bool,
        fut: ExecutionFut<'static>,
        classifier: &'static Classifier<'static, T, DB>,
    ) -> eyre::Result<BlockTree<Action>> {
        let (traces, header) = fut.await?.ok_or_else(|| eyre!("no traces found"))?;

        trace!("Got {} traces + header", traces.len());
        let res = classifier
            .build_block_tree(traces, header, generate_pricing)
            .await;

        Ok(res)
    }

    pub fn fetch_state_for(&mut self, block: u64) {
        let execute_fut = self.parser.execute(block);
        let generate_pricing = self.metadata_fetcher.generate_dex_pricing(block, self.db);
        self.collection_future = Some(Box::pin(
            Self::state_future(generate_pricing, execute_fut, self.classifier)
                .instrument(span!(Level::ERROR, "mev processor", block_number=%block)),
        ))
    }

    pub fn range_finished(&self) {
        self.mark_as_finished.store(true, SeqCst);
    }
}

impl<T: TracingProvider, DB: LibmdbxReader + DBWriter, CH: ClickhouseHandle> Stream
    for StateCollector<T, DB, CH>
{
    type Item = (BlockTree<Action>, Metadata);

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        if let Some(mut collection_future) = self.collection_future.take() {
            match collection_future.poll_unpin(cx) {
                Poll::Ready(Ok(tree)) => {
                    let db = self.db;
                    self.metadata_fetcher.load_metadata_for_tree(tree, db);
                    cx.waker().wake_by_ref();
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

        if self.mark_as_finished.load(SeqCst)
            && self.metadata_fetcher.is_finished()
            && self.collection_future.is_none()
        {
            return Poll::Ready(None)
        }

        self.metadata_fetcher.poll_next_unpin(cx)
    }
}
