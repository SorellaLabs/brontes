use std::{pin::Pin, time::Instant};

use metrics::{Counter, Gauge, Histogram};
use prometheus::{register_int_counter_vec, IntCounterVec, Opts};
use reth_metrics::Metrics;

#[derive(Clone)]
pub struct GlobalRangeMetrics {
    /// the amount of blocks all inspectors have completed
    pub completed_blocks:       Counter,
    /// the runtime for inspectors
    pub processing_run_time_ms: Histogram,
    ///
    /// complete
    pub completed_blocks_range: IntCounterVec,
    /// the amount of blocks the inspector has completed
    /// the total blocks in the inspector range
    pub total_blocks_range:     IntCounterVec,
}

impl GlobalRangeMetrics {
    pub fn new(ranges: usize, per_range_blocks: &[usize]) -> Self {
        let completed_blocks_range = register_int_counter_vec!(
            "brontes_range_speicifc_completed_blocks",
            "total blocks completed per range",
            &["range_id"],
        )
        .unwrap();

        let total_blocks_range = register_int_counter_vec!(
            "brontes_range_total_blocks",
            "total blocks for specific range",
            &["range_id"]
        )
        .unwrap();

        for (i, block) in per_range_blocks.iter().enumerate() {
            let strd = format!("{i}");
            let res = total_blocks_range
                .get_metric_with_label_values(&[&strd])
                .unwrap();
            res.inc_by(*block as u64);
        }

        Self {
            completed_blocks_range,
            total_blocks_range,
            completed_blocks: metrics::register_counter!("brontes_total_completed_blocks"),
            processing_run_time_ms: metrics::register_histogram!("brontes_processing_runtime_ms"),
        }
    }

    pub fn finished_block(&self, id: usize) {
        let strd = format!("{id}");
        self.completed_blocks_range
            .get_metric_with_label_values(&[&strd])
            .unwrap()
            .inc();

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

#[derive(Clone)]
pub struct RangeMetrics {
    pub completed_blocks: IntCounterVec,
    /// the amount of blocks the inspector has completed
    /// the total blocks in the inspector range
    pub total_blocks:     IntCounterVec,
}

impl RangeMetrics {}

#[derive(Metrics, Clone)]
#[metrics(scope = "brontes_running_ranges")]
pub struct FinishedRange {
    /// the active ranges running
    pub running_ranges:  Gauge,
    /// total amount of blocks. for the set range.
    /// if at tip, then this is the range at init
    pub total_set_range: Counter,
}
