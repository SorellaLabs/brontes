use core::panic;
use std::{collections::VecDeque, pin::Pin, task::Poll};

use brontes_database::clickhouse::ClickhouseHandle;
use brontes_pricing::types::DexPriceMsg;
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
use tokio::sync::mpsc::UnboundedReceiver;

use super::dex_pricing::WaitingForPricerFuture;

/// Limits the amount we work ahead in the processing. This is done
/// as the Pricer is a slow process
const MAX_PENDING_TREES: usize = 20;

pub type ClickhouseMetadataFuture =
    FuturesOrdered<Pin<Box<dyn Future<Output = (u64, BlockTree<Actions>, Metadata)> + Send>>>;

/// deals with all cases on how we get and finalize our metadata
pub struct MetadataFetcher<T: TracingProvider, DB: DBWriter + LibmdbxReader, CH: ClickhouseHandle> {
    clickhouse:         Option<&'static CH>,
    dex_pricer_stream:  Option<WaitingForPricerFuture<T, DB>>,
    /// we will drain this in the case we aren't running a dex pricer to avoid
    /// being terrible on memory
    no_price_chan:      Option<UnboundedReceiver<DexPriceMsg>>,
    clickhouse_futures: ClickhouseMetadataFuture,

    result_buf: VecDeque<(BlockTree<Actions>, Metadata)>,
}

impl<T: TracingProvider, DB: DBWriter + LibmdbxReader, CH: ClickhouseHandle>
    MetadataFetcher<T, DB, CH>
{
    pub fn new(
        clickhouse: Option<&'static CH>,
        dex_pricer_stream: Option<WaitingForPricerFuture<T, DB>>,
        no_price_chan: Option<UnboundedReceiver<DexPriceMsg>>,
    ) -> Self {
        Self {
            clickhouse,
            dex_pricer_stream,
            no_price_chan,
            clickhouse_futures: FuturesOrdered::new(),
            result_buf: VecDeque::new(),
        }
    }

    pub fn should_process_next_block(&self) -> bool {
        self.dex_pricer_stream
            .as_ref()
            .map(|pricer| pricer.pending_trees.len() < MAX_PENDING_TREES)
            .unwrap_or(true)
    }

    pub fn is_finished(&self) -> bool {
        self.result_buf.is_empty()
            && self
                .dex_pricer_stream
                .as_ref()
                .map(|stream| stream.is_done())
                .unwrap_or(true)
            && self.clickhouse_futures.is_empty()
    }

    fn clear_no_price_channel(&mut self) {
        if let Some(chan) = self.no_price_chan.as_mut() {
            while chan.try_recv().is_ok() {}
        }
    }

    pub fn load_metadata_for_tree(&mut self, tree: BlockTree<Actions>, libmdbx: &'static DB) {
        let block = tree.header.number;
        // clear price channel
        self.clear_no_price_channel();
        // pull directly from libmdbx
        if self.dex_pricer_stream.is_none() && self.clickhouse.is_none() {
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
        } else if let Some(clickhouse) = self.clickhouse {
            tracing::debug!(?block, "spawning clickhouse fut");
            let future = Box::pin(async move {
                let mut meta = clickhouse
                    .get_metadata(block)
                    .await
                    .expect("missing metadata for clickhouse.get_metadata request");
                meta.builder_info = libmdbx
                    .try_fetch_builder_info(tree.header.beneficiary)
                    .expect("failed to fetch builder info table in libmdbx");
                (block, tree, meta)
            });
            self.clickhouse_futures.push_back(future);
        } else if let Some(pricer) = self.dex_pricer_stream.as_mut() {
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
            pricer.add_pending_inspection(block, tree, meta);
        } else {
            panic!("metadata fetcher not setup properly")
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
        self.clear_no_price_channel();

        if let Some(res) = self.result_buf.pop_front() {
            return Poll::Ready(Some(res))
        }
        if let Some(mut pricer) = self.dex_pricer_stream.take() {
            while let Poll::Ready(Some((block, tree, meta))) =
                self.clickhouse_futures.poll_next_unpin(cx)
            {
                pricer.add_pending_inspection(block, tree, meta)
            }

            let res = pricer.poll_next_unpin(cx);
            self.dex_pricer_stream = Some(pricer);

            return res
        }

        std::task::Poll::Pending
    }
}
