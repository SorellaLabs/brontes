mod range;
mod shared;
use brontes_database::Tables;
use reth_db::Tables;
mod tip;
use std::{
    collections::HashMap,
    pin::Pin,
    sync::{atomic::AtomicBool, Arc},
    task::{Context, Poll},
};

use alloy_primitives::Address;
use brontes_classifier::Classifier;
use brontes_core::decoding::{Parser, TracingProvider};
use brontes_database::{
    clickhouse::Clickhouse,
    libmdbx::{LibmdbxReadWriter, LibmdbxReader, LibmdbxWriter},
};
use brontes_inspect::Inspector;
use brontes_pricing::{BrontesBatchPricer, GraphManager};
use futures::{future, future::join_all, stream::FuturesUnordered, Future, StreamExt};
use itertools::Itertools;
pub use range::RangeExecutorWithPricing;
use reth_tasks::TaskExecutor;
pub use tip::TipInspector;
use tokio::{sync::mpsc::unbounded_channel, task::JoinHandle};

use self::shared::{
    dex_pricing::WaitingForPricerFuture, metadata::MetadataFetcher, state_collector::StateCollector,
};
use crate::cli::static_object;

pub const PROMETHEUS_ENDPOINT_IP: [u8; 4] = [127u8, 0u8, 0u8, 1u8];
pub const PROMETHEUS_ENDPOINT_PORT: u16 = 6423;

pub struct BrontesRunConfig<T: TracingProvider> {
    pub start_block:      u64,
    pub end_block:        Option<u64>,
    pub max_tasks:        u64,
    pub min_batch_size:   u64,
    pub quote_asset:      Address,
    pub with_dex_pricing: bool,

    pub inspectors: &'static [&'static dyn Inspector],
    pub clickhouse: &'static Clickhouse,
    pub parser:     &'static Parser<'static, T, LibmdbxReadWriter>,
    pub libmdbx:    &'static LibmdbxReadWriter,
}

impl<T: TracingProvider> BrontesRunConfig<T> {
    pub fn new(
        start_block: u64,
        end_block: Option<u64>,

        max_tasks: u64,
        min_batch_size: u64,
        quote_asset: Address,
        with_dex_pricing: bool,

        inspectors: &'static [&'static dyn Inspector],
        clickhouse: Option<&'static Clickhouse>,

        parser: &'static Parser<'static, T, LibmdbxReadWriter>,
        libmdbx: &'static LibmdbxReadWriter,
    ) -> Self {
        Self {
            clickhouse,
            start_block,
            min_batch_size,
            max_tasks,
            with_dex_pricing,
            parser,
            libmdbx,
            inspectors,
            quote_asset,
            end_block,
        }
    }

    async fn build_range_executors(
        &self,
        executor: TaskExecutor,
        end_block: u64,
    ) -> Vec<RangeExecutorWithPricing<T, LibmdbxReadWriter>> {
        // calculate the chunk size using min batch size and max_tasks.
        // max tasks defaults to 25% of physical threads of the system if not set
        let range = end_block - self.start_block;
        let cpus_min = range / self.min_batch_size + 1;

        let cpus = std::cmp::min(cpus_min, self.max_tasks);
        let chunk_size = if cpus == 0 { range + 1 } else { (range / cpus) + 1 };

        join_all(
            futures::stream::iter(self.start_block..=end_block)
                .chunks(chunk_size.try_into().unwrap())
                .enumerate()
                .map(|(batch_id, chunk)| {
                    let executor = executor.clone();
                    async move {
                        let start_block = chunk.first().unwrap();
                        let end_block = chunk.last().unwrap_or(start_block);

                        tracing::info!(batch_id, start_block, end_block, "starting batch");

                        let state_collector = if self.with_dex_pricing {
                            self.state_collector_dex_price(
                                executor.clone(),
                                *start_block,
                                *end_block,
                            )
                            .await
                        } else {
                            self.state_collector_no_dex_price()
                        };

                        RangeExecutorWithPricing::new(
                            *start_block,
                            *end_block,
                            state_collector,
                            self.libmdbx,
                            self.inspectors,
                        )
                    }
                })
                .collect::<Vec<_>>()
                .await,
        )
        .await
    }

    async fn build_tip_inspector(
        &self,
        executor: TaskExecutor,
        start_block: u64,
    ) -> TipInspector<T, LibmdbxReadWriter> {
        let state_collector = self
            .state_collector_dex_price(executor, start_block, start_block)
            .await;
        TipInspector::new(
            start_block,
            self.quote_asset,
            state_collector,
            self.parser,
            self.libmdbx,
            self.inspectors,
        )
    }

    fn state_collector_no_dex_price(&self) -> StateCollector<T, LibmdbxReadWriter> {
        let (tx, rx) = unbounded_channel();
        let classifier = static_object(Classifier::new(self.libmdbx, tx, self.parser.get_tracer()));

        let fetcher = MetadataFetcher::new(None, None, Some(rx));
        StateCollector::new(
            Arc::new(AtomicBool::new(false)),
            fetcher,
            classifier,
            self.parser,
            self.libmdbx,
        )
    }

    async fn state_collector_dex_price(
        &self,
        executor: TaskExecutor,
        start_block: u64,
        end_block: u64,
    ) -> StateCollector<T, LibmdbxReadWriter> {
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
                    .map(|(addr, protocol, pair)| (addr, (protocol, pair)))
                    .collect::<Vec<_>>()
            })
            .collect::<HashMap<_, _>>();

        let pair_graph = GraphManager::init_from_db_state(pairs, HashMap::default(), self.libmdbx);

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
        let fetcher = MetadataFetcher::new(Some(self.clickhouse), Some(pricing), None);

        StateCollector::new(shutdown, fetcher, classifier, self.parser, self.libmdbx)
    }

    pub async fn verify_database_fetch_missing(&self, end_block: u64) -> eyre::Result<()> {
        tracing::info!("initing critical range state");
        // these tables are super lightweight and as such, we init them for the entire
        // range
        self.libmdbx
            .initialize_tables(
                self.clickhouse,
                self.parser.get_tracer(),
                &[
                    Tables::PoolCreationBlocks,
                    Tables::TokenDecimals,
                    Tables::PoolCreationBlocks,
                    Tables::Builder,
                    Tables::AddressMeta,
                ],
                false,
                None,
            )
            .await?;

        tracing::info!(start_block=%self.start_block, %end_block, "verifying db fetching state that is missing");
        let state_to_init = self.libmdbx.state_to_initialize(
            self.start_block,
            end_block,
            !self.with_dex_pricing,
        )?;

        tracing::info!("initing missing ranges");
        join_all(state_to_init.into_iter().map(|range| async {
            let start = range.start();
            let end = range.end();
            self.libmdbx
                .initialize_tables(
                    self.clickhouse,
                    self.parser.get_tracer(),
                    &[Tables::BlockInfo, Tables::CexPrice],
                    false,
                    Some((*start, *end)),
                )
                .await
        }))
        .await
        .into_iter()
        .collect::<eyre::Result<_>>()?;

        Ok(())
    }

    pub async fn build(self, executor: TaskExecutor) -> eyre::Result<Brontes> {
        let futures = FuturesUnordered::new();
        let (had_end_block, end_block) = if let Some(end_block) = self.end_block {
            (true, end_block)
        } else {
            #[cfg(not(feature = "local"))]
            let chain_tip = self.parser.get_latest_block_number().unwrap();
            #[cfg(feature = "local")]
            let chain_tip = self.parser.get_latest_block_number().await.unwrap();
            (false, chain_tip)
        };

        self.verify_database_fetch_missing(end_block).await?;

        if had_end_block {
            (&self)
                .build_range_executors(executor.clone(), end_block)
                .await
                .into_iter()
                .for_each(|block_range| {
                    futures.push(executor.spawn_critical_with_graceful_shutdown_signal(
                        "range_executor",
                        |shutdown| async move {
                            block_range.run_until_graceful_shutdown(shutdown).await
                        },
                    ));
                });
        } else {
            (&self)
                .build_range_executors(executor.clone(), end_block)
                .await
                .into_iter()
                .for_each(|block_range| {
                    futures.push(executor.spawn_critical_with_graceful_shutdown_signal(
                        "range_executor",
                        |shutdown| async move {
                            block_range.run_until_graceful_shutdown(shutdown).await
                        },
                    ));
                });

            let tip_inspector = self.build_tip_inspector(executor.clone(), end_block).await;
            futures.push(executor.spawn_critical_with_graceful_shutdown_signal(
                "Tip Inspector",
                |shutdown| async move { tip_inspector.run_until_graceful_shutdown(shutdown).await },
            ));
        }

        Ok(Brontes(futures))
    }
}

pub struct Brontes(FuturesUnordered<JoinHandle<()>>);

impl Future for Brontes {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.0.is_empty() {
            return Poll::Ready(())
        }

        if let Poll::Ready(None) = self.0.poll_next_unpin(cx) {
            return Poll::Ready(())
        }

        cx.waker().wake_by_ref();
        return Poll::Pending
    }
}
