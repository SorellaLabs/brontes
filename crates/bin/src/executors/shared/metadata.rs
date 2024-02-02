use core::panic;
use std::{collections::VecDeque, pin::Pin, task::Poll};

use brontes_core::{LibmdbxReader, LibmdbxWriter};
use brontes_database::{clickhouse::Clickhouse, libmdbx::types::dex_price};
use brontes_pricing::{types::DexPriceMsg, BrontesBatchPricer, GraphManager};
use brontes_types::{
    db::{
        dex::DexQuotes,
        metadata::{MetadataCombined, MetadataNoDex},
    },
    normalized_actions::Actions,
    traits::TracingProvider,
    BlockTree,
};
use eyre::eyre;
use futures::{stream::FuturesOrdered, Future, Stream, StreamExt};
use reth_tasks::TaskExecutor;
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::error;

use super::dex_pricing::WaitingForPricerFuture;
use crate::executors::dex_pricing::WaitingForPricerFuture;

/// deals with all cases on how we get and finalize our metadata
pub struct MetadataFetcher<T: TracingProvider, DB: LibmdbxWriter + LibmdbxReader> {
    clickhouse:         Option<&'static Clickhouse>,
    dex_pricer_stream:  Option<WaitingForPricerFuture<T, DB>>,
    /// we will drain this in the case we aren't running a dex pricer to avoid
    /// being terrible on memory
    no_price_chan:      Option<UnboundedReceiver<DexPriceMsg>>,
    clickhouse_futures: FuturesOrdered<
        Pin<Box<dyn Future<Output = (u64, BlockTree<Actions>, MetadataNoDex)> + Send + Sync>>,
    >,

    result_buf: VecDeque<(BlockTree<Actions>, MetadataCombined)>,
}

impl<T: TracingProvider, DB: LibmdbxWriter + LibmdbxReader> MetadataFetcher<T, DB> {
    pub fn new(
        clickhouse: Option<&'static Clickhouse>,
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

    pub fn running_pricing(&self) -> bool {
        self.dex_pricer_stream.is_some()
    }

    pub fn get_price_channel(&mut self) -> Option<UnboundedReceiver<DexPriceMsg>> {
        self.no_price_chan.take()
    }

    pub fn into_tip_mode(
        &mut self,
        pricer: BrontesBatchPricer<T, DB>,
        clickhouse: &'static Clickhouse,
        exector: TaskExecutor,
    ) {
        self.clickhouse = Some(clickhouse);
        self.dex_pricer_stream = Some(WaitingForPricerFuture::new(pricer, exector));
    }

    fn clear_no_price_chan(&mut self) {
        if let Some(chan) = self.no_price_chan.as_mut() {
            while let Ok(_) = chan.try_recv() {}
        }
    }

    pub fn load_metadata_for_tree(&mut self, tree: BlockTree<Actions>, libmdbx: &'static DB) {
        let block = tree.header.number;
        // clear price channel
        self.clear_no_price_chan();
        // pull directly from libmdbx
        if self.dex_pricer_stream.is_none() && self.clickhouse.is_none() {
            let meta = libmdbx.get_metadata(block)?;
            self.result_buf.push_back((tree, meta));
        // need to pull the metadata from clickhouse
        } else if let Some(clickhouse) = self.clickhouse {
            let future = Box::pin(async move {
                let meta = clickhouse.get_metadata(block).await;
                (block, tree, meta)
            });
            self.clickhouse_futures.push_back(future);
        // don't need to pull from clickhouse, means we are running pricing
        } else if let Some(pricer) = self.dex_pricer_stream.as_mut() {
            let meta = libmdbx.get_metadata_no_dex(block)?;
            pricer.add_pending_inspection(block, tree, meta);
        } else {
            panic!("metadata fetcher not setup properly")
        }
    }
}

impl<T: TracingProvider, DB: LibmdbxReader + LibmdbxWriter> Stream for MetadataFetcher<T, DB> {
    type Item = (BlockTree<Actions>, MetadataCombined);

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.clear_no_price_chan();

        if let Some(res) = self.result_buf.pop_front() {
            return Poll::Ready(Some(res))
        }
        if let Some(pricer) = self.dex_pricer_stream.as_mut() {
            while let Poll::Ready(Some((block, tree, meta))) =
                self.clickhouse_futures.poll_next_unpin(cx)
            {
                pricer.add_pending_inspection(block, tree, meta)
            }

            return pricer.poll_next_unpin(cx)
        }

        return std::task::Poll::Pending
    }
}
