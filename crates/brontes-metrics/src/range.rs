use std::{pin::Pin, time::Instant};

use metrics::{Counter, Gauge, Histogram};
use reth_metrics::Metrics;

#[derive(Metrics, Clone)]
#[metrics(scope = "range_executor")]
pub struct GlobalRangeMetrics {
    /// the amount of blocks the inspector has completed
    pub completed_blocks:       Counter,
    /// the runtime for inspectors
    pub processing_run_time_ms: Histogram,
}

impl GlobalRangeMetrics {
    pub fn finished_block(&self) {
        self.completed_blocks.increment(1);
    }

    pub async fn meter_processing<R>(
        self,
        f: impl FnOnce() -> Pin<Box<dyn futures::Future<Output = R> + Send>>,
    ) -> R {
        let time = Instant::now();
        let res = f().await;
        let elapsed = time.elapsed().as_millis() as f64;
        self.processing_run_time_ms.record(elapsed);

        res
    }
}

#[derive(Metrics, Clone)]
#[metrics(dynamic = true)]
pub struct RangeMetrics {
    /// the amount of blocks the inspector has completed
    pub completed_blocks: Counter,
    /// the total blocks in the inspector range
    pub total_blocks:     Counter,
}

impl RangeMetrics {
    pub fn finished_block(&self) {
        self.completed_blocks.increment(1);
    }
}

#[derive(Metrics, Clone)]
#[metrics(scope = "brontes_running_ranges")]
pub struct FinishedRange {
    /// the active ranges running
    pub running_ranges:  Gauge,
    /// total amount of blocks. for the set range.
    /// if at tip, then this is the range at init
    pub total_set_range: Counter,
}
