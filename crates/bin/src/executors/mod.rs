mod block;
mod dex_pricing;
mod error;
mod range;
mod tip;
mod utils;

use std::{
    pin::Pin,
    task::{Context, Poll},
};

use alloy_primitives::Address;
pub use block::BlockInspector;
use brontes_classifier::Classifier;
use brontes_core::decoding::{Parser, TracingProvider};
use brontes_database::{
    clickhouse::Clickhouse,
    libmdbx::{LibmdbxReader, LibmdbxWriter},
};
use brontes_inspect::Inspector;
use brontes_pricing::types::DexPriceMsg;
use futures::{pin_mut, stream::FuturesUnordered, Future, FutureExt, StreamExt};
pub use range::RangeExecutorWithPricing;
use reth_tasks::{shutdown::GracefulShutdown, TaskExecutor};
pub use tip::TipInspector;
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::info;

pub const PROMETHEUS_ENDPOINT_IP: [u8; 4] = [127u8, 0u8, 0u8, 1u8];
pub const PROMETHEUS_ENDPOINT_PORT: u16 = 6423;

enum Mode {
    Historical,
    Tip,
}

pub struct Brontes<'inspector, T: TracingProvider, DB: LibmdbxReader + LibmdbxWriter> {
    current_block:    u64,
    end_block:        Option<u64>,
    chain_tip:        u64,
    quote_asset:      Address,
    mode:             Mode,
    max_tasks:        u64,
    parser:           &'inspector Parser<'inspector, T, DB>,
    classifier:       &'inspector Classifier<'inspector, T, DB>,
    inspectors:       &'inspector [&'inspector Box<dyn Inspector>],
    clickhouse:       &'inspector Clickhouse,
    database:         &'static DB,
    block_inspectors: FuturesUnordered<BlockInspector<'inspector, T, DB>>,
    tip_inspector:    Option<TipInspector<'inspector, T, DB>>,
    task_executor:    TaskExecutor,
    dex_price_rx:     Option<UnboundedReceiver<DexPriceMsg>>,
}

impl<'inspector, T: TracingProvider, DB: LibmdbxWriter + LibmdbxReader> Brontes<'inspector, T, DB> {
    pub fn new(
        init_block: u64,
        end_block: Option<u64>,
        chain_tip: u64,
        max_tasks: u64,
        parser: &'inspector Parser<'inspector, T, DB>,
        clickhouse: &'inspector Clickhouse,
        database: &'static DB,
        classifier: &'inspector Classifier<'_, T, DB>,
        inspectors: &'inspector [&'inspector Box<dyn Inspector>],
        task_executor: TaskExecutor,
        dex_price_rx: UnboundedReceiver<DexPriceMsg>,
        quote_asset: Address,
    ) -> Self {
        let mut brontes = Self {
            current_block: init_block,
            end_block,
            chain_tip,
            mode: Mode::Historical,
            max_tasks,
            parser,
            clickhouse,
            database,
            classifier,
            inspectors,
            block_inspectors: FuturesUnordered::new(),
            tip_inspector: None,
            task_executor,
            dex_price_rx: Some(dex_price_rx),
            quote_asset,
        };

        let max_blocks = match end_block {
            Some(end_block) => end_block.min(init_block + max_tasks),
            None => init_block + max_tasks,
        };

        for _ in init_block..=max_blocks {
            brontes.spawn_block_inspector();
        }

        brontes
    }

    pub async fn run_until_graceful_shutdown(self, shutdown: GracefulShutdown) {
        let brontes = self;
        pin_mut!(brontes, shutdown);

        let mut graceful_guard = None;
        tokio::select! {
            _= &mut brontes=> {

            },
            guard = shutdown => {
                graceful_guard = Some(guard);
            },
        }
        // finish all block inspectors
        while let Some(_) = brontes.block_inspectors.next().await {}
        if let Some(tip) = brontes.tip_inspector.take() {
            tip.shutdown().await;
        }

        info!("brontes properly shutdown");

        drop(graceful_guard);
    }

    fn spawn_block_inspector(&mut self) {
        let inspector = BlockInspector::new(
            self.parser,
            self.database,
            self.classifier,
            self.inspectors,
            self.current_block,
        );
        info!(block_number = self.current_block, "started new block inspector");
        self.current_block += 1;
        self.block_inspectors.push(inspector);
    }

    fn spawn_tip_inspector(&mut self) {
        let mut rx = self.dex_price_rx.take().unwrap();
        // drain all historical
        while let Ok(_) = rx.try_recv() {}

        let inspector = TipInspector::new(
            self.parser,
            self.clickhouse,
            self.database,
            self.classifier,
            self.inspectors,
            self.chain_tip,
            self.task_executor.clone(),
            rx,
            self.quote_asset,
        );

        info!(block_number = self.chain_tip, "Finished historical inspectors, now tracking tip");
        self.tip_inspector = Some(inspector);
    }

    fn start_block_inspector(&mut self) -> bool {
        // reached end of line
        if self.block_inspectors.len() >= self.max_tasks as usize
            || Some(self.current_block) > self.end_block
        {
            return false
        }

        #[cfg(not(feature = "local"))]
        if self.current_block >= self.chain_tip {
            if let Ok(chain_tip) = self.parser.get_latest_block_number() {
                if chain_tip > self.chain_tip {
                    self.chain_tip = chain_tip;
                } else {
                    self.mode = Mode::Tip;
                    self.spawn_tip_inspector();
                    return false
                }
            }
        }

        #[cfg(feature = "local")]
        if self.current_block >= self.chain_tip {
            if let Ok(chain_tip) = tokio::task::block_in_place(|| {
                // This will now run the future to completion on the current thread
                // without blocking the entire runtime
                futures::executor::block_on(self.parser.get_latest_block_number())
            }) {
                self.chain_tip = chain_tip;
            } else {
                // no new block ready
                return false
            }
        }

        true
    }

    fn progress_block_inspectors(&mut self, cx: &mut Context<'_>) {
        while let Poll::Ready(Some(_)) = self.block_inspectors.poll_next_unpin(cx) {}
    }
}

impl<T: TracingProvider, DB: LibmdbxReader + LibmdbxWriter> Future for Brontes<'_, T, DB> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut iters = 256;
        loop {
            match self.mode {
                Mode::Historical => {
                    if Some(self.current_block) >= self.end_block
                        && self.block_inspectors.is_empty()
                    {
                        return Poll::Ready(())
                    }

                    if self.start_block_inspector() {
                        self.spawn_block_inspector();
                    }

                    self.progress_block_inspectors(cx);
                }
                Mode::Tip => {
                    if let Some(tip_inspector) = self.tip_inspector.as_mut() {
                        match tip_inspector.poll_unpin(cx) {
                            Poll::Ready(()) => return Poll::Ready(()),
                            Poll::Pending => {}
                        }
                    }
                }
            }

            iters -= 1;
            if iters == 0 {
                cx.waker().wake_by_ref();
                return Poll::Pending
            }
        }
    }
}
