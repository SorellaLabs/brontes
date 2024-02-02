use std::collections::VecDeque;

use brontes_core::{LibmdbxReader, LibmdbxWriter};
use brontes_database::{clickhouse::Clickhouse, libmdbx::types::dex_price};
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
use futures::{Stream, StreamExt};

use crate::executors::dex_pricing::WaitingForPricerFuture;

/// deals with all cases on how we get and finalize our metadata
pub struct MetadataFetcher<T: TracingProvider, DB: LibmdbxWriter + LibmdbxReader> {
    clickhouse:        Option<&'static Clickhouse>,
    dex_pricer_stream: Option<WaitingForPricerFuture<T, DB>>,
    /// we will drain this in the case we aren't running a dex pricer to avoid
    /// being terrible on memory
    no_price_chan:     Option<UnboundedReceiver<DexPriceMsg>>,
}

impl<T: TracingProvider, DB: LibmdbxWriter + LibmdbxReader> MetadataFinalizer<T, DB> {
    pub fn new(
        clickhouse: Option<&'static Clickhouse>,
        dex_pricer_stream: Option<WaitingForPricerFuture<T, DB>>,
        no_price_chan: Option<UnboundedReceiver<DexPriceMsg>>,
    ) -> Self {
        todo!()
    }

    fn clear_no_price_chan(&mut self) {
        if let Some(chan) = self.no_price_chan.as_mut() {
            while let Ok(_) = chan.try_recv() {}
        }
    }

    pub async fn load_metadata_for_tree(
        &mut self,
        block: u64,
        tree: BlockTree<Actions>,
        libmdbx: &'static DB,
    ) -> eyre::Result<(BlockTree<Actions>, MetadataCombined)> {
        // clear price channel
        self.clear_no_price_chan();
        // pull directly from libmdbx
        if self.dex_pricer_stream.is_none() && self.clickhouse.is_none() {
            let meta = libmdbx.get_metadata(block)?;
            return Ok((tree, meta))

        // need to pull the metadata from clickhouse
        } else if let Some(clickhouse) = self.clickhouse {
            let meta = clickhouse.get_metadata(block).await;
            return Ok((tree, meta))
        // don't need to pull from clickhouse, means we are running pricing
        } else if let Some(pricer) = self.dex_pricer_stream.as_mut() {
            let meta = libmdbx.get_metadata_no_dex(block)?;
            pricer.add_pending_inspection(block, tree, meta);
            return Ok(pricer.next().await)
        } else {
            return Err(eyre!("metadata fetcher is not setup correctly"))
        }
    }
}
