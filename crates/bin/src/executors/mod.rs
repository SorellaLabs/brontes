mod processors;
mod range;
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
use futures::{future::join_all, stream::FuturesUnordered, Future, StreamExt};
use itertools::Itertools;
pub use range::RangeExecutorWithPricing;
use reth_tasks::shutdown::GracefulShutdown;
pub use tip::TipInspector;
use tokio::{sync::mpsc::unbounded_channel, task::JoinHandle, time::Duration};

use self::shared::{
    dex_pricing::WaitingForPricerFuture, metadata::MetadataFetcher, state_collector::StateCollector,
};
use crate::cli::static_object;

pub const PROMETHEUS_ENDPOINT_IP: [u8; 4] = [127u8, 0u8, 0u8, 1u8];
pub const PROMETHEUS_ENDPOINT_PORT: u16 = 6423;

pub struct BrontesRunConfig<T: TracingProvider, DB: LibmdbxInit, CH: ClickhouseHandle, P: Processor>
{
    pub start_block:       Option<u64>,
    pub end_block:         Option<u64>,
    pub back_from_tip:     u64,
    pub max_tasks:         u64,
    pub min_batch_size:    u64,
    pub quote_asset:       Address,
    pub force_dex_pricing: bool,
    pub only_cex_dex:      bool,

    pub inspectors: &'static [&'static dyn Inspector<Result = P::InspectType>],
    pub clickhouse: &'static CH,
    pub parser:     &'static Parser<'static, T, DB>,
    pub libmdbx:    &'static DB,
    _p:             PhantomData<P>,
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
        only_cex_dex: bool,

        inspectors: &'static [&'static dyn Inspector<Result = P::InspectType>],
        clickhouse: &'static CH,

        parser: &'static Parser<'static, T, DB>,
        libmdbx: &'static DB,
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
            only_cex_dex,
            _p: PhantomData,
        }
    }

    #[allow(clippy::async_yields_async)]
    async fn build_range_executors(
        &self,
        executor: BrontesTaskExecutor,
        end_block: u64,
        progress_bar: Option<ProgressBar>,
    ) -> Vec<RangeExecutorWithPricing<T, DB, CH, P>> {
        // calculate the chunk size using min batch size and max_tasks.
        // max tasks defaults to 25% of physical threads of the system if not set
        let range = end_block - self.start_block.unwrap();
        let cpus_min = range / self.min_batch_size + 1;

        let cpus = std::cmp::min(cpus_min, self.max_tasks);
        let chunk_size = if cpus == 0 { range + 1 } else { (range / cpus) + 1 };

        join_all(
            (self.start_block.unwrap()..=end_block)
                .chunks(chunk_size.try_into().unwrap())
                .into_iter()
                .enumerate()
                .map(|(batch_id, mut chunk)| {
                    let executor = executor.clone();
                    let start_block = chunk.next().unwrap();
                    let end_block = chunk.last().unwrap_or(start_block);

                    let prgrs_bar = progress_bar.clone();

                    async move {
                        tracing::info!(batch_id, start_block, end_block, "Starting batch");

                        RangeExecutorWithPricing::new(
                            start_block,
                            end_block,
                            self.init_state_collector(
                                executor.clone(),
                                start_block,
                                end_block,
                                false,
                            )
                            .await,
                            self.libmdbx,
                            self.inspectors,
                            prgrs_bar,
                        )
                    }
                }),
        )
        .await
    }

    async fn build_tip_inspector(
        &self,
        executor: BrontesTaskExecutor,
        start_block: u64,
        back_from_tip: u64,
    ) -> TipInspector<T, DB, CH, P> {
        let state_collector = self
            .init_state_collector(executor, start_block, start_block, true)
            .await;
        TipInspector::new(
            start_block,
            back_from_tip,
            state_collector,
            self.parser,
            self.libmdbx,
            self.inspectors,
        )
    }

    async fn init_state_collector(
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
            self.only_cex_dex,
        );

        StateCollector::new(shutdown, fetcher, classifier, self.parser, self.libmdbx)
    }

    async fn verify_database_fetch_missing(&self, end_block: u64) -> eyre::Result<()> {
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
                )
                .await?;
        }

        if let Some(start_block) = self.start_block {
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

            join_all(state_to_init_continuous.iter().map(|range| async move {
                let start = range.start();
                let end = range.end();

                #[cfg(feature = "sorella-server")]
                {
                    self.libmdbx
                        .initialize_tables(
                            self.clickhouse,
                            self.parser.get_tracer(),
                            &[Tables::BlockInfo, Tables::CexPrice],
                            false,
                            Some((*start, *end)),
                        )
                        .await
                }
                #[cfg(not(feature = "sorella-server"))]
                {
                    self.libmdbx
                        .initialize_tables(
                            self.clickhouse,
                            self.parser.get_tracer(),
                            &[Tables::BlockInfo, Tables::CexPrice, Tables::TxTraces],
                            false,
                            Some((*start, *end)),
                        )
                        .await
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

            #[cfg(feature = "sorella-server")]
            self.libmdbx
                .initialize_tables_arbitrary(
                    self.clickhouse,
                    self.parser.get_tracer(),
                    &[Tables::BlockInfo, Tables::CexPrice],
                    state_to_init_disc,
                )
                .await?;
            #[cfg(not(feature = "sorella-server"))]
            self.libmdbx
                .initialize_tables_arbitrary(
                    self.clickhouse,
                    self.parser.get_tracer(),
                    &[Tables::BlockInfo, Tables::CexPrice, Tables::TxTraces],
                    state_to_init_disc,
                )
                .await?;

            Ok(())
        } else {
            Ok(())
        }
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

        let progress_bar = if self.start_block.is_some() && had_end_block {
            let total_blocks = end_block - self.start_block.unwrap();
            let progress_bar = ProgressBar::with_draw_target(
                Some(total_blocks),
                ProgressDrawTarget::stderr_with_hz(1),
            );
            progress_bar.set_style(
                ProgressStyle::with_template(
                    "{msg}\n[{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} blocks \
                     ({percent}%) | ETA: {eta}",
                )?
                .progress_chars("█>-")
                .with_key("eta", |state: &ProgressState, f: &mut dyn std::fmt::Write| {
                    write!(f, "{:.1}s", state.eta().as_secs_f64()).unwrap()
                })
                .with_key(
                    "percent",
                    |state: &ProgressState, f: &mut dyn std::fmt::Write| {
                        write!(f, "{:.1}", state.fraction() * 100.0).unwrap()
                    },
                ),
            );
            progress_bar.set_message("Processing blocks:");
            Some(progress_bar)
        } else {
            None
        };

        if had_end_block && self.start_block.is_some() {
            self.build_range_executors(executor.clone(), end_block, progress_bar.clone())
                .await
                .into_iter()
                .for_each(|block_range| {
                    futures.push(executor.spawn_critical_with_graceful_shutdown_signal(
                        "Range Executor",
                        |shutdown| async move {
                            block_range.run_until_graceful_shutdown(shutdown).await
                        },
                    ));
                });
        } else {
            if self.start_block.is_some() {
                self.build_range_executors(executor.clone(), end_block, progress_bar.clone())
                    .await
                    .into_iter()
                    .for_each(|block_range| {
                        futures.push(executor.spawn_critical_with_graceful_shutdown_signal(
                            "Range Executor",
                            |shutdown| async move {
                                block_range.run_until_graceful_shutdown(shutdown).await
                            },
                        ));
                    });
            }
            let tip_inspector = self
                .build_tip_inspector(executor.clone(), end_block, self.back_from_tip)
                .await;

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
        self.verify_database_fetch_missing(end_block).await?;
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
