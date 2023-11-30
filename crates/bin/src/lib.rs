use std::{
    pin::Pin,
    task::{Context, Poll},
};

use brontes_classifier::Classifier;
use brontes_core::decoding::{Parser, TracingProvider};
use brontes_database::database::Database;
use brontes_inspect::Inspector;
use futures::{stream::FuturesUnordered, Future, StreamExt};
use tracing::info;

mod block_inspector;
use block_inspector::BlockInspector;

pub const PROMETHEUS_ENDPOINT_IP: [u8; 4] = [127u8, 0u8, 0u8, 1u8];
pub const PROMETHEUS_ENDPOINT_PORT: u16 = 6423;

// composer created for each block
// need to have a tracker of end block or tip block
// need a concept of batch size

pub struct Brontes<'inspector, const N: usize, T: TracingProvider> {
    current_block:    u64,
    end_block:        Option<u64>,
    chain_tip:        u64,
    max_tasks:        u64,
    parser:           &'inspector Parser<'inspector, T>,
    classifier:       &'inspector Classifier,
    inspectors:       &'inspector [&'inspector Box<dyn Inspector>; N],
    database:         &'inspector Database,
    block_inspectors: FuturesUnordered<BlockInspector<'inspector, N, T>>,
}

impl<'inspector, const N: usize, T: TracingProvider> Brontes<'inspector, N, T> {
    pub fn new(
        init_block: u64,
        end_block: Option<u64>,
        chain_tip: u64,
        max_tasks: u64,
        parser: &'inspector Parser<'inspector, T>,
        database: &'inspector Database,
        classifier: &'inspector Classifier,
        inspectors: &'inspector [&'inspector Box<dyn Inspector>; N],
    ) -> Self {
        let mut poirot = Self {
            current_block: init_block,
            end_block,
            chain_tip,
            max_tasks,
            parser,
            database,
            classifier,
            inspectors,
            block_inspectors: FuturesUnordered::new(),
        };

        let max_blocks = match end_block {
            Some(end_block) => end_block.min(init_block + max_tasks),
            None => init_block + max_tasks,
        };

        for _ in init_block..max_blocks {
            poirot.spawn_block_inspector();
        }
        poirot
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

    fn start_block_inspector(&mut self) -> bool {
        // reached end of line
        if self.block_inspectors.len() > self.max_tasks as usize
            || Some(self.current_block + self.max_tasks) > self.end_block
        {
            return false
        }

        #[cfg(feature = "server")]
        if self.current_block >= self.chain_tip {
            if let Ok(chain_tip) = self.parser.get_latest_block_number() {
                self.chain_tip = chain_tip;
            } else {
                // no new block ready
                return false
            }
        }

        #[cfg(not(feature = "server"))]
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

impl<const N: usize, T: TracingProvider> Future for Brontes<'_, N, T> {
    type Output = ();

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
            if let Some(end_block) = self.end_block {
                if self.current_block > end_block {
                    if self.block_inspectors.is_empty() && self.current_block > end_block {
                        return Poll::Ready(())
                    }
                }
            }

            if self.start_block_inspector() {
                self.spawn_block_inspector();
            }

            self.progress_block_inspectors(cx);

            iters -= 1;
            if iters == 0 {
                cx.waker().wake_by_ref();
                break
            }
        }

        Poll::Pending
    }
}
