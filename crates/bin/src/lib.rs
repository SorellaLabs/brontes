use std::{
    pin::Pin,
    task::{Context, Poll},
};

use brontes_classifier::Classifier;
use brontes_core::decoding::{Parser, TracingProvider};
use brontes_database::database::Database;
use brontes_inspect::Inspector;
use futures::{stream::FuturesUnordered, Future, StreamExt};

mod block_inspector;
use block_inspector::BlockInspector;

pub const PROMETHEUS_ENDPOINT_IP: [u8; 4] = [127u8, 0u8, 0u8, 1u8];
pub const PROMETHEUS_ENDPOINT_PORT: u16 = 6423;

// composer created for each block
// need to have a tracker of end block or tip block
// need a concept of batch size

pub struct Poirot<'inspector, const N: usize, T: TracingProvider> {
    current_block:    u64,
    end_block:        Option<u64>,
    chain_tip:        u64,
    max_tasks:        usize,
    parser:           &'inspector Parser<T>,
    classifier:       &'inspector Classifier,
    inspectors:       &'inspector [&'inspector Box<dyn Inspector>; N],
    database:         &'inspector Database,
    block_inspectors: FuturesUnordered<BlockInspector<'inspector, N, T>>,
}

impl<'inspector, const N: usize, T: TracingProvider> Poirot<'inspector, N, T> {
    pub fn new(
        init_block: u64,
        end_block: Option<u64>,
        chain_tip: u64,
        max_tasks: usize,
        parser: &'inspector Parser<T>,
        database: &'inspector Database,
        classifier: &'inspector Classifier,
        inspectors: &'inspector [&'inspector Box<dyn Inspector>; N],
    ) -> Self {
        Self {
            current_block: init_block,
            end_block,
            chain_tip,
            max_tasks,
            parser,
            database,
            classifier,
            inspectors,
            block_inspectors: FuturesUnordered::new(),
        }
    }

    fn spawn_block_inspector(&mut self) {
        if self.current_block > self.chain_tip {
            if let Ok(chain_tip) =
                tokio::runtime::Handle::current().block_on(self.parser.get_latest_block_number())
            {
                self.chain_tip = chain_tip;
            } else {
                // no new block ready
                return
            }
        }

        let inspector = BlockInspector::new(
            self.parser,
            self.database,
            self.classifier,
            self.inspectors,
            self.current_block,
        );
        self.block_inspectors.push(inspector);
    }

    fn start_block_inspector(&mut self) -> bool {
        // If we've reached the max number of tasks, we shouldn't spawn a new one
        if self.block_inspectors.len() >= self.max_tasks {
            return false
        }

        self.current_block += 1;
        true
    }

    fn progress_block_inspectors(&mut self, cx: &mut Context<'_>) {
        while let Poll::Ready(Some(_)) = self.block_inspectors.poll_next_unpin(cx) {}
    }
}

impl<const N: usize, T: TracingProvider> Future for Poirot<'_, N, T> {
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
            // We could instantiate the max amount of block inspectors here, but
            // I have decided to let the system breathe a little. You people should be
            // compassionate to your machines. They have feelings too. Roko's
            // basilisk. also see: https://www.youtube.com/watch?v=lhMWNhpjmpo

            if let Some(end_block) = self.end_block {
                if self.current_block > end_block {
                    return Poll::Ready(())
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
