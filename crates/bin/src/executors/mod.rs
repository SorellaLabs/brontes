mod processors;
mod range;
use futures::Stream;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressState, ProgressStyle};
pub use processors::*;
mod shared;
use brontes_database::{clickhouse::ClickhouseHandle, Tables};
use futures::pin_mut;
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
use brontes_types::{BrontesTaskExecutor, FastHashMap};
use futures::{stream::FuturesUnordered, Future, StreamExt};
use indicatif::MultiProgress;
use itertools::Itertools;
pub use range::RangeExecutorWithPricing;
use reth_tasks::shutdown::GracefulShutdown;
pub use tip::TipInspector;
use tokio::{sync::mpsc::unbounded_channel, task::JoinHandle};

use self::shared::{
    dex_pricing::WaitingForPricerFuture, metadata::MetadataFetcher, state_collector::StateCollector,
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
    pub parser: &'static Parser<'static, T, DB>,
    pub libmdbx: &'static DB,
    pub cli_only: bool,
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
        parser: &'static Parser<'static, T, DB>,
        libmdbx: &'static DB,
        cli_only: bool,
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
            _p: PhantomData,
        }
    }

    //TODO: We currently don't have the ability to stream the query results from
    // clickhouse because the client is shit, so we have to break up the downloads
    // into smaller batches & wait for these smaller queries to return to write all
    // of the data. This uses a lot of memory & is slow. We will switch to using
    // stream functionality.
    fn build_range_executors(
        &'_ self,
        executor: BrontesTaskExecutor,
        has_end_block: bool,
        end_block: u64,
    ) -> impl Stream<Item = RangeExecutorWithPricing<T, DB, CH, P>> + '_ {
        // calculate the chunk size using min batch size and max_tasks.
        // max tasks defaults to 25% of physical threads of the system if not set
        let range = end_block - self.start_block.unwrap();
        let cpus_min = range / self.min_batch_size + 1;

        let cpus = std::cmp::min(cpus_min, self.max_tasks);
        let chunk_size = if cpus == 0 { range + 1 } else { (range / cpus) + 1 };

        let start_block = self.start_block.unwrap();

        let total_blocks = 12;
        let global_pb = self.initialize_global_progress_bar(has_end_block, end_block, total_blocks);

        let state_to_init = self.libmdbx.state_to_initialize(start_block, end_block)?;

        let multi = MultiProgress::default();
        let tables_with_progress = Arc::new(
            tables
                .into_iter()
                .map(|table| (table, table.build_init_state_progress_bar(&multi, 5)))
                .collect_vec(),
        );

        futures::stream::iter(chunks.into_iter().enumerate().map(
            move |(batch_id, (start_block, end_block))| {
                let executor = executor.clone();
                let prgrs_bar = progress_bar.clone();
                let tables_pb = tables_pb.clone();

                #[allow(clippy::async_yields_async)]
                async move {
                    tracing::info!(batch_id, start_block, end_block, "Starting batch");
                    self.init_block_range_tables(start_block, end_block, tables_pb.clone())
                        .await
                        .unwrap();

                    #[allow(clippy::async_yields_async)]
                    RangeExecutorWithPricing::new(
                        start_block,
                        end_block,
                        self.init_state_collector(executor.clone(), start_block, end_block, false),
                        self.libmdbx,
                        self.inspectors,
                        prgrs_bar,
                    )
                }
            },
        ))
        .buffer_unordered(15)
    }

    fn build_tip_inspector(
        &self,
        executor: BrontesTaskExecutor,
        start_block: u64,
        back_from_tip: u64,
    ) -> TipInspector<T, DB, CH, P> {
        let state_collector = self.init_state_collector(executor, start_block, start_block, true);
        TipInspector::new(
            start_block,
            back_from_tip,
            state_collector,
            self.parser,
            self.libmdbx,
            self.inspectors,
        )
    }

    fn init_state_collector(
        &self,
        executor: BrontesTaskExecutor,
        start_block: u64,
        end_block: u64,
        tip: bool,
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

        let pair_graph =
            GraphManager::init_from_db_state(pairs, FastHashMap::default(), self.libmdbx);

        let pricer = BrontesBatchPricer::new(
            shutdown.clone(),
            self.quote_asset,
            pair_graph,
            rx,
            self.parser.get_tracer(),
            start_block,
            rest_pairs,
        );
        let pricing = WaitingForPricerFuture::new(pricer, executor);
        let fetcher = MetadataFetcher::new(
            tip.then_some(self.clickhouse),
            pricing,
            self.force_dex_pricing,
            self.force_no_dex_pricing,
        );

        StateCollector::new(shutdown, fetcher, classifier, self.parser, self.libmdbx)
    }

    async fn init_block_range_tables(
        &self,
        start_block: u64,
        end_block: u64,
        cli_only: bool,
        tables_pb: Arc<Vec<(Tables, ProgressBar)>>,
    ) -> eyre::Result<()> {
        todo!();

        tracing::info!(
            start_block=%start_block, %end_block,
            "Verifying the database contains the data from block {} to block {}",
            start_block,
            end_block
        );

        for keys in state_to_init.iter() {}

        let state_to_init_continuous = state_to_init
            .clone()
            .into_iter()
            .filter(|range| range.clone().collect_vec().len() >= 1000)
            .collect_vec();

        tracing::info!("Downloading missing {:#?} ranges", state_to_init);

        tracing::info!("Downloading {:#?} missing continuous ranges", state_to_init_continuous);

        join_all(state_to_init_continuous.iter().map(|range| {
            let tables_pb = tables_pb.clone();
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
                            tables_pb.clone(),
                        )
                        .await
                }
            }
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
                tables_pb.clone(),
            )
            .await?;

        Ok(())
    }

    /// Verify global tables & initialize them if necessary
    async fn verify_global_tables(&self) -> eyre::Result<()> {
        if self.libmdbx.init_full_range_tables(self.clickhouse).await {
            tracing::info!("Initializing critical range state");
            self.libmdbx
                .initialize_full_range_tables(self.clickhouse, self.parser.get_tracer())
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
    ) -> eyre::Result<Brontes> {
        let futures = FuturesUnordered::new();

        let mut tables = Tables::ALL.to_vec();
        #[cfg(not(feature = "cex-dex-markout"))]
        tables.retain(|t| !matches!(t, Tables::CexTrades));
        #[cfg(feature = "cex-dex-markout")]
        tables.retain(|t| !matches!(t, Tables::CexPrice));

        if had_end_block && self.start_block.is_some() {
            self.build_range_executors(executor.clone(), has_end_block, end_block)
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
        } else {
            if self.start_block.is_some() {
                self.build_range_executors(executor.clone(), has_end_block, end_block)
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
                self.build_tip_inspector(executor.clone(), end_block, self.back_from_tip);

            futures.push(executor.spawn_critical_with_graceful_shutdown_signal(
                "Tip Inspector",
                |shutdown| async move { tip_inspector.run_until_graceful_shutdown(shutdown).await },
            ));
        }

        Ok(Brontes { futures, progress_bar })
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

    fn initialize_global_progress_bar(
        &self,
        has_end_block: bool,
        end_block: u64,
        total_blocks: u64,
    ) -> Option<ProgressBar> {
        self.cli_only.and_then(|| {
            start_block.and_then(|start_block, total_blocks| {
                // Assuming `had_end_block` and `end_block` should be defined or passed
                // elsewhere
                if had_end_block {
                    let progress_bar = ProgressBar::with_draw_target(
                        total_blocks,
                        ProgressDrawTarget::stderr_with_hz(100),
                    );
                    let style = ProgressStyle::default_bar()
                        .template(
                            "{msg}\n[{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} \
                             blocks ({percent}%) | ETA: {eta}",
                        )
                        .expect("Invalid progress bar template")
                        .progress_chars("█>-")
                        .with_key("eta", |state: &ProgressState, f: &mut dyn std::fmt::Write| {
                            write!(f, "{:.1}s", state.eta().as_secs_f64()).unwrap()
                        })
                        .with_key(
                            "percent",
                            |state: &ProgressState, f: &mut dyn std::fmt::Write| {
                                write!(f, "{:.1}", state.fraction() * 100.0).unwrap()
                            },
                        );
                    progress_bar.set_style(style);
                    progress_bar.set_message("Processing blocks:");
                    Some(progress_bar)
                } else {
                    None
                }
            })
        })
    }

    pub fn build_init_state_progress_bar(
        &self,
        multi_progress_bar: &MultiProgress,
        total_blocks: u64,
    ) -> ProgressBar {
        let progress_bar = ProgressBar::with_draw_target(
            Some(total_blocks),
            ProgressDrawTarget::stderr_with_hz(60),
        );
        progress_bar.set_style(
            ProgressStyle::with_template(
                "{msg}\n[{elapsed_precise}] [{wide_bar:.green/red}] {pos}/{len} ({percent}%)",
            )
            .unwrap()
            .progress_chars("#>-")
            .with_key(
                "percent",
                |state: &ProgressState, f: &mut dyn std::fmt::Write| {
                    write!(f, "{:.1}", state.fraction() * 100.0).unwrap()
                },
            ),
        );
        progress_bar.set_message(format!("{}", self));
        multi_progress_bar.add(progress_bar)
    }
}

pub struct Brontes {
    futures:      FuturesUnordered<JoinHandle<()>>,
    progress_bar: Option<ProgressBar>,
}

impl Future for Brontes {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.futures.is_empty() {
            if let Some(bar) = &self.progress_bar {
                bar.finish();
            }
            return Poll::Ready(())
        }

        if let Poll::Ready(None) = self.futures.poll_next_unpin(cx) {
            if let Some(bar) = &self.progress_bar {
                bar.finish();
            }
            return Poll::Ready(())
        }

        cx.waker().wake_by_ref();
        Poll::Pending
    }
}
