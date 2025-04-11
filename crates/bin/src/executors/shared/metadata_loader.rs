use std::{
    collections::VecDeque,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    task::Poll,
    time::Duration,
};

use alloy_primitives::{Address, BlockHash};
use brontes_database::clickhouse::ClickhouseHandle;
use brontes_types::{
    db::{
        cex::trades::{window_loader::CexWindow, CexTradeMap},
        dex::DexQuotes,
        metadata::Metadata,
        traits::{DBWriter, LibmdbxReader},
    },
    normalized_actions::Action,
    traits::TracingProvider,
    BlockData, BlockTree,
};
use futures::{stream::FuturesOrdered, Future, Stream, StreamExt};
use itertools::Itertools;
use tracing::error;

use super::dex_pricing::WaitingForPricerFuture;

/// Limits the amount we work ahead in the processing. This is done
/// as the Pricer is a slow process and otherwise we will end up caching 100+ gb
/// of processed trees
const MAX_PENDING_TREES: usize = 5;

pub type ClickhouseMetadataFuture =
    FuturesOrdered<Pin<Box<dyn Future<Output = (u64, BlockTree<Action>, Metadata)> + Send>>>;

/// deals with all cases on how we get and finalize our metadata
pub struct MetadataLoader<T: TracingProvider, CH: ClickhouseHandle> {
    clickhouse:            Option<&'static CH>,
    dex_pricer_stream:     WaitingForPricerFuture<T>,
    clickhouse_futures:    ClickhouseMetadataFuture,
    result_buf:            VecDeque<BlockData>,
    needs_more_data:       Arc<AtomicBool>,
    cex_window_data:       CexWindow,
    always_generate_price: bool,
    force_no_dex_pricing:  bool,
}

impl<T: TracingProvider, CH: ClickhouseHandle> MetadataLoader<T, CH> {
    pub fn new(
        clickhouse: Option<&'static CH>,
        dex_pricer_stream: WaitingForPricerFuture<T>,
        always_generate_price: bool,
        force_no_dex_pricing: bool,
        needs_more_data: Arc<AtomicBool>,
        #[allow(unused)] cex_window_sec: usize,
    ) -> Self {
        Self {
            cex_window_data: CexWindow::new(cex_window_sec),
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
        !self.force_no_dex_pricing
            && (self.always_generate_price
                || libmdbx
                    .get_dex_quotes(block)
                    .map(|f| f.0.is_empty())
                    .unwrap_or(true))
    }

    pub fn load_metadata_for_tree<DB: LibmdbxReader + DBWriter>(
        &mut self,
        block_hash: BlockHash,
        tree: BlockTree<Action>,
        libmdbx: &'static DB,
        quote_asset: Address,
    ) {
        let block = tree.header.number;
        let generate_dex_pricing = self.generate_dex_pricing(block, libmdbx);

        if !generate_dex_pricing && self.clickhouse.is_none() {
            self.load_metadata_with_dex_prices(tree, libmdbx, block, quote_asset);
        } else if let Some(clickhouse) = self.clickhouse {
            self.load_metadata_from_clickhouse(
                tree,
                libmdbx,
                clickhouse,
                block,
                block_hash,
                quote_asset,
            );
        } else if self.force_no_dex_pricing {
            self.load_metadata_force_no_dex_pricing(tree, libmdbx, block, quote_asset);
        } else {
            self.load_metadata_no_dex_pricing(tree, libmdbx, block, quote_asset);
        }
    }

    fn load_cex_trades<DB: LibmdbxReader>(
        &mut self,
        libmdbx: &'static DB,
        block: u64,
    ) -> Option<CexTradeMap> {
        if !self.cex_window_data.is_loaded() {
            let window = self.cex_window_data.get_window_lookahead();
            // given every download is -6 + 6 around the block
            // we calculate the offset from the current block that we need
            let offsets = (window / 12) as u64;
            let mut trades = Vec::new();
            for block in block - offsets..=block + offsets {
                if let Ok(res) = libmdbx.get_cex_trades(block) {
                    trades.push(res);
                }
            }
            let last_block = block + offsets;
            self.cex_window_data.init(last_block, trades);

            return Some(self.cex_window_data.cex_trade_map());
        }

        let last_block = self.cex_window_data.get_last_end_block_loaded() + 1;

        if let Ok(res) = libmdbx.get_cex_trades(last_block) {
            self.cex_window_data.new_block(res);
        }
        self.cex_window_data.set_last_block(last_block);

        Some(self.cex_window_data.cex_trade_map())
    }

    fn load_metadata_no_dex_pricing<DB: LibmdbxReader>(
        &mut self,
        tree: BlockTree<Action>,
        libmdbx: &'static DB,
        block: u64,
        quote_asset: Address,
    ) {
        // pull metadata from libmdbx and generate dex_pricing
        let Ok(mut meta) = libmdbx
            .get_metadata_no_dex_price(block, quote_asset)
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

        meta.cex_trades = self.load_cex_trades(libmdbx, block);

        tracing::debug!(?block, "waiting for dex price");

        self.dex_pricer_stream
            .add_pending_inspection(block, tree, meta);
    }

    fn load_metadata_force_no_dex_pricing<DB: LibmdbxReader>(
        &mut self,
        tree: BlockTree<Action>,
        libmdbx: &'static DB,
        block: u64,
        quote_asset: Address,
    ) {
        tracing::debug!(?block, "only cex dex. skipping dex pricing");
        let Ok(mut meta) = libmdbx
            .get_metadata_no_dex_price(block, quote_asset)
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

        let mut meta = meta.into_full_metadata(DexQuotes(vec![]));
        meta.cex_trades = self.load_cex_trades(libmdbx, block);

        self.result_buf
            .push_back(BlockData { metadata: meta.into(), tree: tree.into() });
    }

    /// loads the full metadata including dex pricing from libmdbx
    fn load_metadata_with_dex_prices<DB: LibmdbxReader>(
        &mut self,
        tree: BlockTree<Action>,
        libmdbx: &'static DB,
        block: u64,
        quote_asset: Address,
    ) {
        let Ok(mut meta) = libmdbx.get_metadata(block, quote_asset).map_err(|err| {
            tracing::error!(%err);
            err
        }) else {
            tracing::error!(?block, "failed to load full metadata from libmdbx");
            self.dex_pricer_stream.add_failed_tree(block);
            return;
        };
        meta.builder_info = libmdbx
            .try_fetch_builder_info(tree.header.beneficiary)
            .expect("failed to fetch builder info table in libmdbx");

        meta.cex_trades = self.load_cex_trades(libmdbx, block);

        tracing::debug!(?block, "caching result buf");
        self.result_buf
            .push_back(BlockData { metadata: meta.into(), tree: tree.into() });
    }

    fn load_metadata_from_clickhouse<DB: LibmdbxReader + DBWriter>(
        &mut self,
        tree: BlockTree<Action>,
        libmdbx: &'static DB,
        clickhouse: &'static CH,
        block: u64,
        block_hash: BlockHash,
        quote_asset: Address,
    ) {
        tracing::info!(?block, "spawning clickhouse fut");
        let window = self.cex_window_data.get_window_lookahead();
        // given every download is -6 + 6 around the block
        // we calculate the offset from the current block that we need
        let offsets = (window / 12) as u64;
        let future = Box::pin(async move {
            let builder_info = libmdbx
                .try_fetch_builder_info(tree.header.beneficiary)
                .expect("failed to fetch builder info table in libmdbx");

            //fetch metadata till it works
            let mut meta = loop {
                if let Ok(res) = clickhouse
                    .get_metadata(
                        block,
                        tree.header.timestamp,
                        block_hash,
                        tree.get_hashes(),
                        quote_asset,
                    )
                    .await
                    .inspect_err(|e| {
                        error!(err=?e);
                    })
                {
                    break res;
                } else {
                    tracing::warn!(
                        ?block,
                        "failed to load block meta from clickhouse. waiting a second and then \
                         trying again"
                    );
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            };

            // fetch trades till it works
            let trades = loop {
                if let Ok(ranges) = clickhouse
                    .get_cex_trades(
                        brontes_database::libmdbx::cex_utils::CexRangeOrArbitrary::Range(
                            block - offsets,
                            block + offsets,
                        ),
                    )
                    .await
                    .inspect_err(|e| {
                        error!(err=?e);
                    })
                {
                    let mut trades = CexTradeMap::default();
                    for range in ranges.into_iter().sorted_unstable_by_key(|k| k.key) {
                        trades.merge_in_map(range.value);
                    }

                    break trades;
                } else {
                    tracing::warn!(
                        ?block,
                        "failed to load trades from clickhouse. waiting a second and then trying \
                         again"
                    );
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            };

            meta.cex_trades = Some(trades);
            meta.builder_info = builder_info;
            (block, tree, meta)
        });

        self.clickhouse_futures.push_back(future);
    }
}

impl<T: TracingProvider, CH: ClickhouseHandle> Stream for MetadataLoader<T, CH> {
    type Item = BlockData;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        if self.force_no_dex_pricing {
            if let Some(res) = self.result_buf.pop_front() {
                return Poll::Ready(Some(res));
            }
            cx.waker().wake_by_ref();
            return Poll::Pending;
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
