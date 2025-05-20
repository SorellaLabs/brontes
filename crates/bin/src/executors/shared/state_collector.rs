use std::{
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering::SeqCst},
        Arc,
    },
    task::{Poll, Waker},
};

use alloy_primitives::Address;
use brontes_classifier::Classifier;
use brontes_core::decoding::Parser;
use brontes_database::clickhouse::ClickhouseHandle;
use brontes_metrics::range::GlobalRangeMetrics;
use brontes_types::{
    db::traits::{DBWriter, LibmdbxReader},
    normalized_actions::Action,
    structured_trace::TxTrace,
    traits::TracingProvider,
    BlockTree, MultiBlockData,
};
use eyre::eyre;
use futures::{Future, FutureExt, Stream, StreamExt};
use reth_primitives::{BlockHash, Header};
use tracing::{span, trace, Instrument, Level};

use super::{metadata_loader::MetadataLoader, multi_block_window::MultiBlockWindow};

type CollectionFut<'a> =
    Pin<Box<dyn Future<Output = eyre::Result<(BlockHash, BlockTree<Action>)>> + Send + 'a>>;
type ExecutionFut<'a> =
    Pin<Box<dyn Future<Output = Option<(BlockHash, Vec<TxTrace>, Header)>> + Send + 'a>>;

pub struct StateCollector<T: TracingProvider, DB: LibmdbxReader + DBWriter, CH: ClickhouseHandle> {
    mark_as_finished: Arc<AtomicBool>,
    metadata_fetcher: MetadataLoader<T, CH>,
    classifier:       &'static Classifier<'static, T, DB>,
    parser:           &'static Parser<T, DB>,
    db:               &'static DB,

    collection_future: Option<CollectionFut<'static>>,
    multi_block:       MultiBlockWindow,
    quote_asset:       Address,
}

impl<T: TracingProvider, DB: LibmdbxReader + DBWriter, CH: ClickhouseHandle>
    StateCollector<T, DB, CH>
{
    pub fn new(
        mark_as_finished: Arc<AtomicBool>,
        metadata_fetcher: MetadataLoader<T, CH>,
        classifier: &'static Classifier<'static, T, DB>,
        parser: &'static Parser<T, DB>,
        db: &'static DB,
        multi_block: MultiBlockWindow,
        quote_asset: Address,
    ) -> Self {
        Self {
            mark_as_finished,
            metadata_fetcher,
            classifier,
            parser,
            db,
            collection_future: None,
            multi_block,
            quote_asset,
        }
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
        block: u64,
        fut: ExecutionFut<'static>,
        classifier: &'static Classifier<'static, T, DB>,
        id: usize,
        metrics: Option<GlobalRangeMetrics>,
    ) -> eyre::Result<(BlockHash, BlockTree<Action>)> {
        let Some((block_hash, traces, header)) = fut.await else {
            classifier.block_load_failure(block);
            return Err(eyre!("no traces found {block}"))
        };

        trace!("Got {} traces + header", traces.len());

        let res = if let Some(metrics) = metrics {
            metrics.add_pending_tree(id);
            let txs_count = traces.len();
            metrics
                .tree_builder(id, || {
                    Box::pin(tokio::spawn(classifier.build_block_tree(
                        traces,
                        header,
                        generate_pricing,
                    )))
                })
                .await
                .unwrap()
        } else {
            tokio::spawn(classifier.build_block_tree(traces, header, generate_pricing))
                .await
                .unwrap()
        };

        Ok((block_hash, res))
    }

    pub fn fetch_state_for(&mut self, block: u64, id: usize, metrics: Option<GlobalRangeMetrics>) {
        let execute_fut = self.parser.execute(block, id, metrics.clone());

        let generate_pricing = self.metadata_fetcher.generate_dex_pricing(block, self.db);
        self.collection_future = Some(Box::pin(
            Self::state_future(generate_pricing, block, execute_fut, self.classifier, id, metrics)
                .instrument(span!(Level::ERROR, "mev processor", block_number=%block)),
        ))
    }

    pub fn range_finished(&self, waker: &Waker) {
        if !self.mark_as_finished.swap(true, SeqCst) {
            waker.wake_by_ref();
        }
    }
}

impl<T: TracingProvider, DB: LibmdbxReader + DBWriter, CH: ClickhouseHandle> Stream
    for StateCollector<T, DB, CH>
{
    type Item = MultiBlockData;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        if let Some(mut collection_future) = self.collection_future.take() {
            match collection_future.poll_unpin(cx) {
                Poll::Ready(Ok((block_hash, tree))) => {
                    let db = self.db;
                    let quote_asset = self.quote_asset;
                    self.metadata_fetcher
                        .load_metadata_for_tree(block_hash, tree, db, quote_asset);

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

        self.metadata_fetcher
            .poll_next_unpin(cx)
            .map(|inner| inner.map(|data| self.multi_block.new_block_data(data)))
    }
}
