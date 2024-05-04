use std::{pin::Pin, time::Instant};

use metrics::{Counter, Gauge, Histogram};
use reth_metrics::Metrics;

#[derive(Metrics, Clone)]
#[metrics(scope = "range_executor")]
pub struct RangeMetrics {
    /// the amount of blocks the inspector has completed
    pub completed_blocks:       Counter,
    /// the runtime for inspectors
    pub processing_run_time_ms: Histogram,
}

impl RangeMetrics {
    pub fn finished_block(&self) {
        self.completed_blocks.increment(1);
    }

    pub async fn meter_processing<R>(
        &self,
        f: impl FnOnce() -> Pin<Box<dyn futures::Future<Output = R> + Send>>,
    ) -> R {
        let time = Instant::now();
        let res = f().await;
        let elapsed = time.elapsed().as_millis() as f64;
        self.processing_run_time_ms.record(elapsed);

        res
    }
}
