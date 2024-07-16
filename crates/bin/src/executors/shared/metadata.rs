use core::panic;
use std::{
    collections::VecDeque,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    task::Poll,
};

use brontes_database::clickhouse::ClickhouseHandle;
use brontes_types::{
    db::{dex::DexQuotes, metadata::Metadata, traits::LibmdbxReader},
    normalized_actions::Action,
    traits::TracingProvider,
    BlockData, BlockTree,
};
use futures::{stream::FuturesOrdered, Future, Stream, StreamExt};

use super::dex_pricing::WaitingForPricerFuture;

/// Limits the amount we work ahead in the processing. This is done
/// as the Pricer is a slow process
const MAX_PENDING_TREES: usize = 5;

pub type ClickhouseMetadataFuture =
    FuturesOrdered<Pin<Box<dyn Future<Output = (u64, BlockTree<Action>, Metadata)> + Send>>>;

/// deals with all cases on how we get and finalize our metadata
pub struct MetadataFetcher<T: TracingProvider, CH: ClickhouseHandle> {
    clickhouse:            Option<&'static CH>,
    dex_pricer_stream:     WaitingForPricerFuture<T>,
    clickhouse_futures:    ClickhouseMetadataFuture,
    result_buf:            VecDeque<BlockData>,
    needs_more_data:       Arc<AtomicBool>,
    always_generate_price: bool,
    force_no_dex_pricing:  bool,
    cex_window_seconds:    usize,
}

impl<T: TracingProvider, CH: ClickhouseHandle> Drop for MetadataFetcher<T, CH> {
    fn drop(&mut self) {
        tracing::debug!(buf = self.result_buf.len(), "result buffer metadata fetcher");
    }
}

impl<T: TracingProvider, CH: ClickhouseHandle> MetadataFetcher<T, CH> {
    pub fn new(
        clickhouse: Option<&'static CH>,
        dex_pricer_stream: WaitingForPricerFuture<T>,
        always_generate_price: bool,
        force_no_dex_pricing: bool,
        needs_more_data: Arc<AtomicBool>,
        cex_window_seconds: usize,
    ) -> Self {
        Self {
            cex_window_seconds,
            clickhouse,
            dex_pricer_stream,
            needs_more_data,
            clickhouse_futures: FuturesOrdered::new(),
            result_buf: VecDeque::new(),
            always_generate_price,
            force_no_dex_pricing,
        }
    }

    pub fn should_process_next_block(&self) -> bool {
        self.needs_more_data.load(Ordering::SeqCst)
            && self.dex_pricer_stream.pending_trees() < MAX_PENDING_TREES
            && self.result_buf.len() < MAX_PENDING_TREES
    }

    pub fn is_finished(&self) -> bool {
        self.result_buf.is_empty()
            && self.dex_pricer_stream.is_done()
            && self.clickhouse_futures.is_empty()
    }

    pub fn generate_dex_pricing<DB: LibmdbxReader>(
        &self,
        block: u64,
        libmdbx: &'static DB,
    ) -> bool {
        (self.always_generate_price
            || libmdbx
                .get_dex_quotes(block)
                .map(|f| f.0.is_empty())
                .unwrap_or(true))
            && !self.force_no_dex_pricing
    }

    pub fn load_metadata_for_tree<DB: LibmdbxReader>(
        &mut self,
        tree: BlockTree<Action>,
        libmdbx: &'static DB,
    ) {
        let block = tree.header.number;
        let generate_dex_pricing = self.generate_dex_pricing(block, libmdbx);

        // pull full meta from libmdbx
        if !generate_dex_pricing && self.clickhouse.is_none() {
            let Ok(mut meta) = libmdbx
                .get_metadata(block, self.cex_window_seconds)
                .map_err(|err| {
                    tracing::error!(%err);
                    err
                })
            else {
                tracing::error!(?block, "failed to load full metadata from libmdbx");
                self.dex_pricer_stream.add_failed_tree(block);
                return;
            };
            meta.builder_info = libmdbx
                .try_fetch_builder_info(tree.header.beneficiary)
                .expect("failed to fetch builder info table in libmdbx");

            tracing::debug!(?block, "caching result buf");
            self.result_buf.push_back((tree, meta));
        // pull metadata from clickhouse and generate dex_pricing
        } else if let Some(clickhouse) = self.clickhouse {
            tracing::debug!(?block, "spawning clickhouse fut");
            let future = Box::pin(async move {
                let mut meta = clickhouse.get_metadata(block).await.unwrap_or_else(|_| {
                    panic!("missing metadata for clickhouse.get_metadata request {block}")
                });

                meta.builder_info = libmdbx
                    .try_fetch_builder_info(tree.header.beneficiary)
                    .expect("failed to fetch builder info table in libmdbx");
                (block, tree, meta)
            });
            self.clickhouse_futures.push_back(future);
        } else if self.force_no_dex_pricing {
            tracing::debug!(?block, "only cex dex. skipping dex pricing");
            let Ok(mut meta) = libmdbx
                .get_metadata_no_dex_price(block, self.cex_window_seconds)
                .map_err(|err| {
                    tracing::error!(%err);
                    err
                })
            else {
                self.dex_pricer_stream.add_failed_tree(block);
                tracing::error!(?block, "failed to load metadata no dex price from libmdbx");
                return;
            };
            meta.builder_info = libmdbx
                .try_fetch_builder_info(tree.header.beneficiary)
                .expect("failed to fetch builder info table in libmdbx");

            let meta = meta.into_full_metadata(DexQuotes(vec![]));
            self.result_buf.push_back((tree, meta));
        } else {
            // pull metadata from libmdbx and generate dex_pricing
            let Ok(mut meta) = libmdbx
                .get_metadata_no_dex_price(block, self.cex_window_seconds)
                .map_err(|err| {
                    tracing::error!(%err);
                    err
                })
            else {
                self.dex_pricer_stream.add_failed_tree(block);
                tracing::error!(?block, "failed to load metadata no dex price from libmdbx");
                return;
            };
            meta.builder_info = libmdbx
                .try_fetch_builder_info(tree.header.beneficiary)
                .expect("failed to fetch builder info table in libmdbx");

            tracing::debug!(?block, "waiting for dex price");

            self.dex_pricer_stream
                .add_pending_inspection(block, tree, meta);
        }
    }
}

impl<T: TracingProvider, CH: ClickhouseHandle> Stream for MetadataFetcher<T, CH> {
    type Item = BlockData;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        if self.force_no_dex_pricing {
            if let Some(res) = self.result_buf.pop_front() {
                return Poll::Ready(Some(res))
            }
            cx.waker().wake_by_ref();
            return Poll::Pending
        }

        while let Poll::Ready(Some((block, tree, meta))) =
            self.clickhouse_futures.poll_next_unpin(cx)
        {
            tracing::info!("clickhouse future resolved");
            self.dex_pricer_stream
                .add_pending_inspection(block, tree, meta)
        }

        match self.dex_pricer_stream.poll_next_unpin(cx) {
            Poll::Ready(Some((tree, metadata))) => Poll::Ready(Some(BlockData {
                metadata: Arc::new(metadata),
                tree:     Arc::new(tree),
            })),
            Poll::Ready(None) => Poll::Ready(self.result_buf.pop_front()),
            Poll::Pending => {
                if let Some(f) = self.result_buf.pop_front() {
                    Poll::Ready(Some(f))
                } else {
                    Poll::Pending
                }
            }
        }
    }
}
