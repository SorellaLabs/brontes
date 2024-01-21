//TODO: (Ludwig) Finish this once all other crates have been documented.

//! # Brontes
//!
//! This is the main binary crate for the Brontes project. It uses several other
//! crates in the workspace, which are documented separately:
//!
//! - [brontes-core](../brontes_core/index.html): Tracing for the Brontes
//!   project.
//! - [brontes-inspect](../brontes_inspect/index.html): Mev Inspectors for MEV
//!   detection.
//! - [brontes-types](../brontes_types/index.html): Defines the main types used
//!   across Brontes.
//! - [brontes-classifier](../brontes_classifier/index.html): Contains the
//!   classifier logic pertaining to transaction trace classification &
//!   normalization.
//! - [brontes-metrics](../brontes_metrics/index.html): Handles metrics
//!   collection and reporting.
//! - [brontes-pricing](../brontes_pricing/index.html): Handles DEX pricing
//! - [brontes-database](../brontes_database/index.html): Handles database
//!   related functionalities.
//! - [brontes-database-libmdbx](../brontes_database_libmdbx/index.html):
//!   Provides libmdbx database functionalities for our local libmdbx database.
//! - [reth-tracing-ext](../reth_tracing_ext/index.html): Provides extended
//!   tracing capabilities to match transaction traces to their corresponding
//!   logs.
//!
//! Please refer to the individual crate documentation for more details.

mod cli;

use std::{
    pin::Pin,
    task::{Context, Poll},
};

use brontes_classifier::Classifier;
use brontes_core::decoding::{Parser, TracingProvider};
use brontes_database::{clickhouse::Clickhouse, libmdbx::Libmdbx};
use brontes_inspect::Inspector;
use futures::{stream::FuturesUnordered, Future, FutureExt, StreamExt};
use tracing::info;

mod banner;
mod block_inspector;
#[allow(unused_imports)]
mod tip_inspector;

mod data_batching;
use block_inspector::BlockInspector;
pub use data_batching::DataBatching;
use tip_inspector::TipInspector;

pub const PROMETHEUS_ENDPOINT_IP: [u8; 4] = [127u8, 0u8, 0u8, 1u8];
pub const PROMETHEUS_ENDPOINT_PORT: u16 = 6423;

enum Mode {
    Historical,
    Tip,
}

pub struct Brontes<'inspector, T: TracingProvider> {
    current_block:    u64,
    end_block:        Option<u64>,
    chain_tip:        u64,
    mode:             Mode,
    max_tasks:        u64,
    parser:           &'inspector Parser<'inspector, T>,
    classifier:       &'inspector Classifier<'inspector, T>,
    inspectors:       &'inspector [&'inspector Box<dyn Inspector>],
    clickhouse:       &'inspector Clickhouse,
    database:         &'inspector Libmdbx,
    block_inspectors: FuturesUnordered<BlockInspector<'inspector, T>>,
    tip_inspector:    Option<TipInspector<'inspector, T>>,
}

impl<'inspector, T: TracingProvider> Brontes<'inspector, T> {
    pub fn new(
        init_block: u64,
        end_block: Option<u64>,
        chain_tip: u64,
        max_tasks: u64,
        parser: &'inspector Parser<'inspector, T>,
        clickhouse: &'inspector Clickhouse,
        database: &'inspector Libmdbx,
        classifier: &'inspector Classifier<'_, T>,
        inspectors: &'inspector [&'inspector Box<dyn Inspector>],
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
        let inspector = TipInspector::new(
            self.parser,
            self.clickhouse,
            self.database,
            self.classifier,
            self.inspectors,
            self.chain_tip,
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

impl<T: TracingProvider> Future for Brontes<'_, T> {
    type Output = ();

    //TODO: Fix this comment
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // This loop drives the entire state of network and does a lot of work.
        // Under heavy load (many messages/events), data may arrive faster than it can
        // be processed (incoming messages/requests -> events), and it is
        // possible that more data has already arrived by the time an internal
        // event is processed. Which could turn this loop into a busy loop.
        // Without yielding back to the executor, it can starve other tasks waiting on
        // that executor to execute them, or drive underlying resources To prevent this,
        // we preemptively return control when the `budget` is exhausted. The
        // value itself is chosen somewhat arbitrarily, it is high enough so the
        // swarm can make meaningful progress but low enough that this loop does
        // not starve other tasks for too long. If the budget is exhausted we
        // manually yield back control to the (coop) scheduler. This manual yield point should prevent situations where polling appears to be frozen. See also <https://tokio.rs/blog/2020-04-preemption>
        // And tokio's docs on cooperative scheduling <https://docs.rs/tokio/latest/tokio/task/#cooperative-scheduling>
        let mut iters = 1024;
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
                break
            }
        }
        Poll::Pending
    }
}
