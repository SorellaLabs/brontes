use core::panic;
use std::{collections::VecDeque, pin::Pin, task::Poll};

use brontes_database::clickhouse::ClickhouseHandle;
use brontes_types::{
    db::{
        metadata::Metadata,
        traits::{DBWriter, LibmdbxReader},
    },
    normalized_actions::Actions,
    traits::TracingProvider,
    BlockTree,
};
use futures::{stream::FuturesOrdered, Future, Stream, StreamExt};

use super::dex_pricing::WaitingForPricerFuture;

/// Limits the amount we work ahead in the processing. This is done
/// as the Pricer is a slow process
const MAX_PENDING_TREES: usize = 20;

pub type ClickhouseMetadataFuture =
    FuturesOrdered<Pin<Box<dyn Future<Output = (u64, BlockTree<Actions>, Metadata)> + Send>>>;

/// deals with all cases on how we get and finalize our metadata
pub struct MetadataFetcher<T: TracingProvider, DB: DBWriter + LibmdbxReader, CH: ClickhouseHandle> {
    clickhouse:            Option<&'static CH>,
    dex_pricer_stream:     WaitingForPricerFuture<T, DB>,
    clickhouse_futures:    ClickhouseMetadataFuture,
    result_buf:            VecDeque<(BlockTree<Actions>, Metadata)>,
    always_generate_price: bool,
}

impl<T: TracingProvider, DB: DBWriter + LibmdbxReader, CH: ClickhouseHandle>
    MetadataFetcher<T, DB, CH>
{
    pub fn new(
        clickhouse: Option<&'static CH>,
        dex_pricer_stream: WaitingForPricerFuture<T, DB>,
        always_generate_price: bool,
    ) -> Self {
        Self {
            clickhouse,
            dex_pricer_stream,
            clickhouse_futures: FuturesOrdered::new(),
            result_buf: VecDeque::new(),
            always_generate_price,
        }
    }

    pub fn should_process_next_block(&self) -> bool {
        self.dex_pricer_stream.pending_trees.len() < MAX_PENDING_TREES
    }

    pub fn is_finished(&self) -> bool {
        self.result_buf.is_empty()
            && self.dex_pricer_stream.is_done()
            && self.clickhouse_futures.is_empty()
    }

    pub fn generate_dex_pricing(&self, block: u64, libmdbx: &'static DB) -> bool {
        self.always_generate_price
            || libmdbx
                .get_dex_quotes(block)
                .map(|f| f.0.is_empty())
                .unwrap_or(true)
    }

    pub fn load_metadata_for_tree(&mut self, tree: BlockTree<Actions>, libmdbx: &'static DB) {
        let block = tree.header.number;
        let generate_dex_pricing = self.generate_dex_pricing(block, libmdbx);

        // pull full meta from libmdbx
        if !generate_dex_pricing && self.clickhouse.is_none() {
            let Ok(mut meta) = libmdbx.get_metadata(block).map_err(|err| {
                tracing::error!(%err);
                err
            }) else {
                tracing::error!(?block, "failed to load full metadata from libmdbx");
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
        } else {
            // pull metadata from libmdbx and generate dex_pricing
            let Ok(mut meta) = libmdbx.get_metadata_no_dex_price(block).map_err(|err| {
                tracing::error!(%err);
                err
            }) else {
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

impl<T: TracingProvider, DB: LibmdbxReader + DBWriter, CH: ClickhouseHandle> Stream
    for MetadataFetcher<T, DB, CH>
{
    type Item = (BlockTree<Actions>, Metadata);

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        if let Some(res) = self.result_buf.pop_front() {
            return Poll::Ready(Some(res))
        }

        while let Poll::Ready(Some((block, tree, meta))) =
            self.clickhouse_futures.poll_next_unpin(cx)
        {
            tracing::info!("clickhouse future resolved");
            self.dex_pricer_stream
                .add_pending_inspection(block, tree, meta)
        }

        self.dex_pricer_stream.poll_next_unpin(cx)
    }
}
