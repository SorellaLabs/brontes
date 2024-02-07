mod range;
mod shared;
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
    libmdbx::{LibmdbxReader, LibmdbxWriter},
};
use brontes_inspect::Inspector;
use brontes_pricing::{BrontesBatchPricer, GraphManager};
use futures::{stream::FuturesUnordered, Future, StreamExt};
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

pub struct BrontesRunConfig<T: TracingProvider, DB: LibmdbxReader + LibmdbxWriter> {
    pub start_block: u64,
    pub end_block:   Option<u64>,

    pub max_tasks:        u64,
    pub min_batch_size:   u64,
    pub quote_asset:      Address,
    pub with_dex_pricing: bool,

    pub inspectors: &'static [&'static dyn Inspector],
    pub clickhouse: Option<&'static Clickhouse>,
    pub parser:     &'static Parser<'static, T, DB>,
    pub libmdbx:    &'static DB,
}

impl<T: TracingProvider, DB: LibmdbxWriter + LibmdbxReader> BrontesRunConfig<T, DB> {
    pub fn new(
        start_block: u64,
        end_block: Option<u64>,

        max_tasks: u64,
        min_batch_size: u64,
        quote_asset: Address,
        with_dex_pricing: bool,

        inspectors: &'static [&'static dyn Inspector],
        clickhouse: Option<&'static Clickhouse>,
        parser: &'static Parser<'static, T, DB>,
        libmdbx: &'static DB,
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

    fn build_range_executors(
        &self,
        executor: TaskExecutor,
        end_block: u64,
    ) -> Vec<RangeExecutorWithPricing<T, DB>> {
        let mut executors = Vec::new();

        // calculate the chunk size using min batch size and max_tasks.
        // max tasks defaults to 25% of physical threads of the system if not set
        let range = end_block - self.start_block;
        let cpus_min = range / self.min_batch_size;

        let cpus = std::cmp::min(cpus_min, self.max_tasks);
        let chunk_size = if cpus == 0 { range + 1 } else { (range / cpus) + 1 };
        for (batch_id, mut chunk) in (self.start_block..=end_block)
            .chunks(chunk_size.try_into().unwrap())
            .into_iter()
            .enumerate()
        {
            let start_block = chunk.next().unwrap();
            let end_block = chunk.last().unwrap_or(start_block);

            tracing::info!(batch_id, start_block, end_block, "starting batch");

            let state_collector = if self.with_dex_pricing {
                self.state_collector_dex_price(executor.clone(), start_block, end_block)
            } else {
                self.state_collector_no_dex_price()
            };

            executors.push(RangeExecutorWithPricing::new(
                self.quote_asset,
                start_block,
                end_block,
                state_collector,
                self.libmdbx,
                self.inspectors,
            ));
        }

        executors
    }

    fn build_tip_inspector(&self, executor: TaskExecutor, start_block: u64) -> TipInspector<T, DB> {
        let state_collector = self.state_collector_dex_price(executor, start_block, start_block);
        TipInspector::new(
            start_block,
            self.quote_asset,
            state_collector,
            self.parser,
            self.libmdbx,
            self.inspectors,
        )
    }

    fn state_collector_no_dex_price(&self) -> StateCollector<T, DB> {
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

    fn state_collector_dex_price(
        &self,
        executor: TaskExecutor,
        start_block: u64,
        end_block: u64,
    ) -> StateCollector<T, DB> {
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
        let fetcher = MetadataFetcher::new(self.clickhouse, Some(pricing), None);

        StateCollector::new(shutdown, fetcher, classifier, self.parser, self.libmdbx)
    }

    pub async fn build(self, executor: TaskExecutor) -> Brontes {
        let futures = FuturesUnordered::new();
        if let Some(end_block) = self.end_block {
            (&self)
                .build_range_executors(executor.clone(), end_block)
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
            #[cfg(not(feature = "local"))]
            let chain_tip = self.parser.get_latest_block_number().unwrap();
            #[cfg(feature = "local")]
            let chain_tip = self.parser.get_latest_block_number().await.unwrap();

            (&self)
                .build_range_executors(executor.clone(), chain_tip)
                .into_iter()
                .for_each(|block_range| {
                    futures.push(executor.spawn_critical_with_graceful_shutdown_signal(
                        "range_executor",
                        |shutdown| async move {
                            block_range.run_until_graceful_shutdown(shutdown).await
                        },
                    ));
                });

            let tip_inspector = self.build_tip_inspector(executor.clone(), chain_tip);
            futures.push(executor.spawn_critical_with_graceful_shutdown_signal(
                "Tip Inspector",
                |shutdown| async move { tip_inspector.run_until_graceful_shutdown(shutdown).await },
            ));
        }

        Brontes(futures)
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
