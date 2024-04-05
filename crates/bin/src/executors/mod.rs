pub mod discovery_only;
mod processors;
mod range;
use std::ops::RangeInclusive;

#[cfg(feature = "sorella-server")]
use brontes_database::libmdbx::StateToInitialize;
use brontes_metrics::{
    pricing::DexPricingMetrics,
    range::{FinishedRange, GlobalRangeMetrics},
};
use futures::{future::join_all, Stream};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressState, ProgressStyle};
pub use processors::*;
mod shared;
use brontes_database::{clickhouse::ClickhouseHandle, Tables};
use futures::pin_mut;
use shared::multi_block_window::MultiBlockWindow;
mod tip;
use std::{
    marker::PhantomData,
    pin::Pin,
    sync::{atomic::AtomicBool, Arc},
    task::{Context, Poll},
};

use alloy_primitives::Address;
use brontes_classifier::Classifier;
use brontes_core::decoding::{Parser, TracingProvider};
use brontes_database::libmdbx::LibmdbxInit;
use brontes_inspect::Inspector;
use brontes_pricing::{BrontesBatchPricer, GraphManager, LoadState};
// TUI related
use brontes_types::mev::events::Action;
use brontes_types::{
    mev::{events::TuiEvents, MevBlock},
    BrontesTaskExecutor, FastHashMap,
};
use futures::{future::join_all, stream::FuturesUnordered, Future, StreamExt};
use indicatif::MultiProgress;
use itertools::Itertools;
pub use range::RangeExecutorWithPricing;
use reth_tasks::shutdown::GracefulShutdown;
pub use tip::TipInspector;
use tokio::{
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
};

use self::shared::{
    dex_pricing::WaitingForPricerFuture, metadata_loader::MetadataLoader,
    state_collector::StateCollector,
};
use crate::cli::static_object;





pub const PROMETHEUS_ENDPOINT_IP: [u8; 4] = [127u8, 0u8, 0u8, 1u8];
pub const PROMETHEUS_ENDPOINT_PORT: u16 = 6423;

pub struct BrontesRunConfig<T: TracingProvider, DB: LibmdbxInit, CH: ClickhouseHandle, P: Processor>
{
    pub start_block: Option<u64>,
    pub end_block: Option<u64>,
    pub back_from_tip: u64,
    pub max_tasks: u64,
    pub min_batch_size: u64,
    pub quote_asset: Address,
    pub force_dex_pricing: bool,
    pub force_no_dex_pricing: bool,
    pub inspectors: &'static [&'static dyn Inspector<Result = P::InspectType>],
    pub clickhouse: &'static CH,
    pub parser: &'static Parser<T, DB>,
    pub libmdbx: &'static DB,
    pub tip_db: &'static DB,
    pub cli_only: bool,
    pub metrics: bool,
    pub is_snapshot: bool,
    pub cex_window: usize,
    _p: PhantomData<P>,
}

impl<T: TracingProvider, DB: LibmdbxInit, CH: ClickhouseHandle, P: Processor>
    BrontesRunConfig<T, DB, CH, P>
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        start_block: Option<u64>,
        end_block: Option<u64>,
        back_from_tip: u64,
        max_tasks: u64,
        min_batch_size: u64,
        quote_asset: Address,
        force_dex_pricing: bool,
        force_no_dex_pricing: bool,
        inspectors: &'static [&'static dyn Inspector<Result = P::InspectType>],
        clickhouse: &'static CH,
        parser: &'static Parser<T, DB>,
        libmdbx: &'static DB,
        tip_db: &'static DB,
        cli_only: bool,
        metrics: bool,
        is_snapshot: bool,
        cex_window: usize,
    ) -> Self {
        Self {
            clickhouse,
            start_block,
            back_from_tip,
            min_batch_size,
            max_tasks,
            force_dex_pricing,
            parser,
            libmdbx,
            inspectors,
            quote_asset,
            end_block,
            force_no_dex_pricing,
            cli_only,
            metrics,
            tip_db,
            is_snapshot,
            cex_window,
            _p: PhantomData,
        }
    }

    pub async fn build(
        self,
        executor: BrontesTaskExecutor,
        shutdown: GracefulShutdown,
    ) -> eyre::Result<Brontes> {
        // we always verify before we allow for any canceling
        let (had_end_block, end_block) = self.get_end_block().await;
        self.verify_global_tables().await?;
        let build_future = self.build_internal(executor.clone(), had_end_block, end_block);

        pin_mut!(build_future, shutdown);
        tokio::select! {
            res = &mut build_future => {
                return res
            },
            guard = shutdown => {
                drop(guard)
            }
        }
        tracing::info!(
            "Received shutdown signal during initialization process, clearing possibly corrupted
                           full range tables"
        );

        Err(eyre::eyre!("shutdown"))
    }

    async fn build_internal(
        self,
        executor: BrontesTaskExecutor,
        had_end_block: bool,
        end_block: u64,
    ) -> eyre::Result<Brontes> {
        let futures = FuturesUnordered::new();

        let pricing_metrics = self.metrics.then(DexPricingMetrics::default);

        if self.is_snapshot {
            let (start_block, db_end_block) = self.libmdbx.get_db_range()?;
            if self.start_block.is_none()
                || !had_end_block
                || self.start_block < Some(start_block)
                || end_block > db_end_block
            {
                eyre::bail!(
                    "the current db snapshot block range is {}-{}, please make sure that your set \
                     range falls within these bounds",
                    start_block,
                    db_end_block
                );
            }
        }

        if had_end_block && self.start_block.is_some() {
            self.build_range_executors(executor.clone(), end_block, pricing_metrics.clone())
                .for_each(|block_range| {
                    futures.push(executor.spawn_critical_with_graceful_shutdown_signal(
                        "Range Executor",
                        |shutdown| async move {
                            block_range.run_until_graceful_shutdown(shutdown).await;
                        },
                    ));
                    std::future::ready(())
                })
                .await;
        } else {
            if self.start_block.is_some() {
                self.build_range_executors(executor.clone(), end_block, pricing_metrics.clone())
                    .for_each(|block_range| {
                        futures.push(executor.spawn_critical_with_graceful_shutdown_signal(
                            "Range Executor",
                            |shutdown| async move {
                                block_range.run_until_graceful_shutdown(shutdown).await;
                            },
                        ));
                        std::future::ready(())
                    })
                    .await;
            }

            tracing::info!("starting tip inspector");
            let tip_inspector = self.build_tip_inspector(
                usize::MAX,
                executor.clone(),
                end_block,
                self.back_from_tip,
                pricing_metrics,
            );

            futures.push(executor.spawn_critical_with_graceful_shutdown_signal(
                "Tip Inspector",
                |shutdown| async move { tip_inspector.run_until_graceful_shutdown(shutdown).await },
            ));
        }

        let metrics = FinishedRange::default();
        metrics.running_ranges.increment(futures.len() as f64);
        metrics
            .total_set_range
            .increment(end_block - self.start_block.unwrap_or(end_block));

        Ok(Brontes { futures, metrics })
    }

    //TODO: We currently don't have the ability to stream the query results from
    //TODO: clickhouse because the client is shit, so we have to break up the
    //TODO: downloads into smaller batches & wait for these smaller queries to
    //TODO: return to write all of the data. This uses a lot of memory & is slow.
    //TODO: We will switch to using stream functionality.

    /// Builds a stream of RangeExecutors for processing blocks.
    ///
    /// This function creates a stream of RangeExecutors, each responsible for
    /// processing a chunk of blocks. It handles the initialization of necessary
    /// components and sets up progress tracking.
    ///
    /// # Arguments
    ///
    /// * `executor` - The task executor for spawning asynchronous tasks.
    /// * `end_block` - The final block to process.
    /// * `pricing_metrics` - Optional metrics for DEX pricing.
    ///
    /// # Returns
    ///
    /// Returns a `Stream` that yields `RangeExecutorWithPricing<T, DB, CH, P>`
    /// instances.
    ///
    /// # Notes
    ///
    /// This function uses `buffer_unordered(8)` to limit concurrent
    /// initialization of RangeExecutors. The actual number of concurrently
    /// running executors is not limited by this buffer.
    fn build_range_executors(
        &'_ self,
        executor: BrontesTaskExecutor,
        end_block: u64,
        //progress_bar: Option<ProgressBar>,
        //tables_pb: Arc<Vec<(Tables, ProgressBar)>>,        
        tui_tx : UnboundedSender<Action>,
    ) -> impl Stream<Item = RangeExecutorWithPricing<T, DB, CH, P>> + '_ {
        let chunks = self.calculate_chunks(end_block);

        let progress_bar =
            initialize_global_progress_bar(self.cli_only, self.start_block, self.end_block);

        let state_to_init = Arc::new(
            self.libmdbx
                .state_to_initialize(self.start_block.unwrap(), end_block)
                .unwrap(),
        );

        // #[cfg(feature = "sorella-server")]
        // let buffer_size = calculate_buffer_size(
        //     &state_to_init,
        //     end_block - self.start_block.unwrap(),
        //     chunks.len() as f64,
        // );
        //
        // #[cfg(not(feature = "sorella-server"))]
        // let buffer_size = 10;

        let multi = MultiProgress::default();
        let tables_pb = Arc::new(
            state_to_init
                .tables_with_init_count()
                .map(|(table, count)| {
                    (table, table.build_init_state_progress_bar(&multi, count as u64))
                })
                .collect_vec(),
        );

        let range_metrics = self.metrics.then(|| {
            GlobalRangeMetrics::new(chunks.iter().map(|(start, end)| end - start).collect_vec())
        });

        futures::stream::iter(chunks.into_iter().enumerate().map(
            move |(batch_id, (start_block, end_block))| {
                let ranges =
                    state_to_init.get_state_for_ranges(start_block as usize, end_block as usize);
                let executor = executor.clone();
                //let prgrs_bar = progress_bar.clone();
               // let tables_pb = tables_pb.clone();
                let tui_tx = tui_tx.clone();
                #[allow(clippy::async_yields_async)]
                async move {
                    tracing::info!(batch_id, start_block, end_block, "Starting batch");
                    //self.init_block_range_tables(start_block, end_block, tables_pb.clone())
                    self.init_block_range_tables(start_block, end_block)
                        .await
                        .unwrap();

                    #[allow(clippy::async_yields_async)]
                    RangeExecutorWithPricing::new(
                        batch_id,
                        start_block,
                        end_block,
                        self.init_state_collector(
                            batch_id,
                            executor.clone(),
                            start_block,
                            end_block,
                            false,
                            pricing_metrics,
                        ),
                        self.libmdbx,
                        self.inspectors,
                        //prgrs_bar,
                        tui_tx
                    )
                }
            },
        ))
        .buffer_unordered(4)
    }

    fn build_tip_inspector(
        &self,
        range_id: usize,
        executor: BrontesTaskExecutor,
        start_block: u64,
        back_from_tip: u64,
        tui_tx: UnboundedSender<Action>,
    ) -> TipInspector<T, DB, CH, P> {
        let state_collector = self.init_state_collector(
            range_id,
            executor,
            start_block,
            start_block,
            true,
            pricing_metrics,
        );
        TipInspector::new(
            start_block,
            back_from_tip,
            state_collector,
            self.parser,
            self.tip_db,
            self.inspectors,
            tui_tx,

        )
    }

    /// Initializes a StateCollector for a specific range of blocks.
    ///
    /// This function sets up the necessary components for collecting state data
    /// over a range of blocks, including classification, pricing, and metadata
    /// fetching.
    ///
    /// # Arguments
    ///
    /// * `range_id` - A unique identifier for this range.
    /// * `executor` - The task executor for spawning asynchronous tasks.
    /// * `start_block` - The first block in the range.
    /// * `end_block` - The last block in the range.
    /// * `tip` - Boolean flag indicating if this is for tip processing.
    /// * `pricing_metrics` - Optional metrics for DEX pricing.
    ///
    /// # Returns
    ///
    /// Returns a `StateCollector<T, DB, CH>` initialized with the specified
    /// parameters.
    fn init_state_collector(
        &self,
        range_id: usize,
        executor: BrontesTaskExecutor,
        start_block: u64,
        end_block: u64,
        tip: bool,
        pricing_metrics: Option<DexPricingMetrics>,
    ) -> StateCollector<T, DB, CH> {
        let shutdown = Arc::new(AtomicBool::new(false));
        let (tx, rx) = unbounded_channel();
        let classifier = static_object(Classifier::new(self.libmdbx, tx, self.parser.get_tracer()));

        let pairs = self.libmdbx.protocols_created_before(start_block).unwrap();

        let rest_pairs = self
            .libmdbx
            .protocols_created_range(start_block + 1, end_block)
            .unwrap()
            .into_iter()
            .flat_map(|(_, pools)| {
                pools
                    .into_iter()
                    .filter(|(_, p, _)| p.has_state_updater())
                    .map(|(addr, protocol, pair)| (addr, (protocol, pair)))
                    .collect::<Vec<_>>()
            })
            .collect::<FastHashMap<_, _>>();

        let pair_graph = GraphManager::init_from_db_state(pairs, pricing_metrics.clone());

        let data_req = Arc::new(AtomicBool::new(true));

        let pricer = BrontesBatchPricer::new(
            range_id,
            shutdown.clone(),
            self.quote_asset,
            pair_graph,
            UnboundedYapperReceiver::new(rx, 100_000, "batch pricer".into()),
            self.parser.get_tracer(),
            start_block,
            rest_pairs,
            data_req.clone(),
            pricing_metrics.clone(),
            executor.clone(),
        );

        let pricing = WaitingForPricerFuture::new(pricer, executor);
        let fetcher = MetadataLoader::new(
            tip.then_some(self.clickhouse),
            pricing,
            self.force_dex_pricing,
            self.force_no_dex_pricing,
            data_req,
            self.cex_window,
        );

        let block_window_size = self
            .inspectors
            .iter()
            .max_by_key(|i| i.block_window())
            .map(|v| v.block_window())
            .expect("no inspectors loaded");

        let window = MultiBlockWindow::new(block_window_size);

        StateCollector::new(
            shutdown,
            fetcher,
            classifier,
            self.parser,
            self.libmdbx,
            window,
            self.quote_asset,
        )
    }

    async fn init_block_range_tables(
        &self,
        start_block: u64,
        end_block: u64,
        //tables_pb: Arc<Vec<(Tables, ProgressBar)>>,
    ) -> eyre::Result<()> {
        tracing::info!(start_block=%start_block, %end_block, "Verifying db fetching state that is missing");
        let state_to_init = self.libmdbx.state_to_initialize(start_block, end_block)?;

        if state_to_init.is_empty() {
            return Ok(())
        }

        tracing::info!("Downloading missing {:#?} ranges", state_to_init);

        let state_to_init_continuous = state_to_init
            .clone()
            .into_iter()
            .filter(|range| range.clone().collect_vec().len() >= 1000)
            .collect_vec();

        tracing::info!("Downloading {:#?} missing continuous ranges", state_to_init_continuous);

        let mut tables_to_init = vec![Tables::BlockInfo];
        #[cfg(not(feature = "sorella-server"))]
        tables_to_init.push(Tables::TxTraces);
        #[cfg(not(feature = "cex-dex-markout"))]
        tables_to_init.push(Tables::CexPrice);
        #[cfg(feature = "cex-dex-markout")]
        tables_to_init.push(Tables::CexTrades);

        let tables_to_init_cont = &tables_to_init.clone();
        join_all(state_to_init_continuous.iter().map(|range| {
            //let tables_pb = tables_pb.clone();
            async move {
                let start = range.start();
                let end = range.end();

                {
                    self.libmdbx
                        .initialize_tables(
                            self.clickhouse,
                            self.parser.get_tracer(),
                            tables_to_init_cont,
                            false,
                            Some((*start, *end)),
                            //tables_pb.clone(),
                        )
                        .await
                }
            }
            futs
        }))
        .await
        .into_iter()
        .collect::<eyre::Result<_>>()?;

        tracing::info!(
            "Downloading {} missing discontinuous ranges",
            state_to_init.len() - state_to_init_continuous.len()
        );

        let state_to_init_disc = state_to_init
            .into_iter()
            .filter(|range| range.clone().collect_vec().len() < 1000)
            .flatten()
            .collect_vec();

        self.libmdbx
            .initialize_tables_arbitrary(
                self.clickhouse,
                self.parser.get_tracer(),
                &tables_to_init,
                state_to_init_disc,
                //tables_pb.clone(),
            )
            .await?;

        Ok(())
    }

    async fn verify_database_fetch_missing(&self) -> eyre::Result<()> {
        // these tables are super lightweight and as such, we init them for the entire
        // range
        if self.libmdbx.init_full_range_tables(self.clickhouse).await {
            tracing::info!("Initializing critical range state");
            self.libmdbx
                .initialize_tables(
                    self.clickhouse,
                    self.parser.get_tracer(),
                    &[
                        Tables::PoolCreationBlocks,
                        Tables::AddressToProtocolInfo,
                        Tables::TokenDecimals,
                        Tables::Builder,
                        Tables::AddressMeta,
                    ],
                    false,
                    None,
                    //Arc::new(vec![]),
                )
                .await?;
        }

        Ok(())
    }

    async fn get_end_block(&self) -> (bool, u64) {
        if let Some(end_block) = self.end_block {
            (true, end_block)
        } else {
            #[cfg(feature = "local-reth")]
            let chain_tip = self.parser.get_latest_block_number().unwrap();
            #[cfg(not(feature = "local-reth"))]
            let chain_tip = self.parser.get_latest_block_number().await.unwrap();

            (false, chain_tip - self.back_from_tip)
        }
    }

    async fn build_internal(
        self,
        executor: BrontesTaskExecutor,
        had_end_block: bool,
        end_block: u64,
        tui_tx: UnboundedSender<Action>,
    ) -> eyre::Result<Brontes> {
        let futures = FuturesUnordered::new();

        /*
        let progress_bar = if self.start_block.is_some() && had_end_block {
            let total_blocks = end_block - self.start_block.unwrap();
            let progress_bar = ProgressBar::with_draw_target(
                Some(end - start),
                ProgressDrawTarget::stderr_with_hz(100),
            );
            let style = ProgressStyle::default_bar()
                .template(
                    "{msg}\n[{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} blocks \
                     ({percent}%) | ETA: {eta}",
                )
                .expect("Invalid progress bar template")
                .progress_chars("█>-")
                .with_key("eta", |state: &ProgressState, f: &mut dyn std::fmt::Write| {
                    write!(f, "{:.1}s", state.eta().as_secs_f64()).unwrap()
                })
                .with_key("percent", |state: &ProgressState, f: &mut dyn std::fmt::Write| {
                    write!(f, "{:.1}", state.fraction() * 100.0).unwrap()
                });
            progress_bar.set_style(style);
            progress_bar.set_message("Processing blocks:");
            Some(progress_bar)
        } else {
            None
        };
*/
        let mut tables = Tables::ALL.to_vec();
        #[cfg(not(feature = "cex-dex-markout"))]
        tables.retain(|t| !matches!(t, Tables::CexTrades));
        #[cfg(feature = "cex-dex-markout")]
        tables.retain(|t| !matches!(t, Tables::CexPrice));


                   // TODO:remove progress bar
/*

        let multi = MultiProgress::default();
        let tables_with_progress = Arc::new(
            tables
                .into_iter()
                .map(|table| (table, table.build_init_state_progress_bar(&multi)))
                .collect_vec(),
        );
*/
        if had_end_block && self.start_block.is_some() {
            self.build_range_executors(
                executor.clone(),
                end_block,
                //progress_bar.clone(),
                //tables_with_progress,
                tui_tx.clone(),
            )
            .for_each(|block_range| {
                futures.push(
                    executor.spawn_critical_with_graceful_shutdown_signal(
                        "Range Executor",
                        |shutdown| async move {
                            block_range.run_until_graceful_shutdown(shutdown).await
                        },
                    ),
                );
                std::future::ready(())
            })
            .await;
        } else {
            if self.start_block.is_some() {
                self.build_range_executors(
                    executor.clone(),
                    end_block,
                   // progress_bar.clone(),
                    //tables_with_progress,
                    tui_tx.clone()
                )
                .for_each(|block_range| {
                    futures.push(executor.spawn_critical_with_graceful_shutdown_signal(
                        "Range Executor",
                        |shutdown| async move {
                            block_range.run_until_graceful_shutdown(shutdown).await
                        },
                    ));
                    std::future::ready(())
                })
                .await;
            }

            let tip_inspector =
                self.build_tip_inspector(executor.clone(), end_block, self.back_from_tip,tui_tx);

            futures.push(executor.spawn_critical_with_graceful_shutdown_signal(
                "Tip Inspector",
                |shutdown| async move { tip_inspector.run_until_graceful_shutdown(shutdown).await },
            ));
        }

        //Ok(Brontes { futures, progress_bar })
        Ok(Brontes { futures })
    }

    pub async fn build(
        self,
        executor: BrontesTaskExecutor,
        shutdown: GracefulShutdown,
        app_tx: UnboundedSender<Action>,
    ) -> eyre::Result<Brontes> {
        // we always verify before we allow for any canceling
        let (had_end_block, end_block) = self.get_end_block().await;
        self.verify_database_fetch_missing().await?;
        let build_future = self.build_internal(executor.clone(), had_end_block, end_block,app_tx);

        pin_mut!(build_future, shutdown);
        tokio::select! {
            res = &mut build_future => {
                return res
            },
            guard = shutdown => {
                drop(guard)
            }
        }
        tracing::info!(
            "Received shutdown signal during initialization process, clearing possibly corrupted
                           full range tables"
        );

        Err(eyre::eyre!("shutdown"))
    }
}

pub struct Brontes {
    futures:      FuturesUnordered<JoinHandle<()>>,
    //progress_bar: Option<ProgressBar>,
}

impl Future for Brontes {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.futures.is_empty() {
            /*
            if let Some(bar) = &self.progress_bar {
                bar.finish();
            }*/
            return Poll::Ready(())
        }

        if let Poll::Ready(None) = self.futures.poll_next_unpin(cx) {
            /*
            if let Some(bar) = &self.progress_bar {
                bar.finish();
            }
            */
            return Poll::Ready(())
        }

        Poll::Pending
    }
}
