use std::{pin::Pin, time::Instant};

use metrics::{Counter, Gauge, Histogram};
use prometheus::{
    register_int_counter_vec, register_int_gauge_vec, HistogramVec, IntCounterVec, IntGaugeVec,
    Opts,
};
use reth_metrics::Metrics;

#[derive(Clone)]
pub struct GlobalRangeMetrics {
    /// the amount of blocks all inspectors have completed
    // pub completed_blocks: Counter,
    /// the runtime for inspectors
    // pub processing_run_time_ms: Histogram,
    /// complete
    pub completed_blocks_range:      IntCounterVec,
    /// the amount of blocks the inspector has completed
    /// the total blocks in the inspector range
    pub total_blocks_range:          IntCounterVec,
    /// range poll rate
    pub poll_rate:                   IntCounterVec,
    /// pending inspector runs
    pub active_inspector_processing: IntGaugeVec,
    pub block_tracing_throughput:    HistogramVec,
    pub classification_throughput:   HistogramVec,
    /// amount of pending trees in dex pricing / metadata fetcher
    pub pending_trees:               IntGaugeVec,
}

impl GlobalRangeMetrics {
    pub fn new(per_range_blocks: Vec<u64>) -> Self {
        let completed_blocks_range = register_int_counter_vec!(
            "brontes_range_specific_completed_blocks",
            "total blocks completed per range",
            &["range_id"],
        )
        .unwrap();

        let total_blocks_range = register_int_counter_vec!(
            "brontes_range_specific_total_blocks",
            "total blocks for specific range",
            &["range_id"]
        )
        .unwrap();

        let poll_rate = register_int_counter_vec!(
            "range_poll_rate",
            "the poll rate for the future of the range",
            &["range_id"]
        )
        .unwrap();

        let active_inspector_processing = register_int_gauge_vec!(
            "range_active_inspector_processing",
            "the amount of inspectors currently running",
            &["range_id"]
        )
        .unwrap();

        for (i, block) in per_range_blocks.into_iter().enumerate() {
            let strd = format!("{i}");
            let res = total_blocks_range
                .get_metric_with_label_values(&[&strd])
                .unwrap();
            res.inc_by(block);
        }

        let buckets = prometheus::exponential_buckets(1.0, 2.0, 22).unwrap();

        let block_tracing = prometheus::register_histogram_vec!(
            "block_tracing_throughput",
            "block tracing speed",
            &["range_id"],
            buckets.clone()
        )
        .unwrap();

        let tree_builder = prometheus::register_histogram_vec!(
            "tree_builder_throughput",
            "tree builder speed ",
            &["range_id"],
            buckets
        )
        .unwrap();

        let pending_trees = register_int_gauge_vec!(
            "range_pending_trees",
            "amount of pending trees in metadata fetcher and dex pricer",
            &["range_id"]
        )
        .unwrap();

        Self {
            pending_trees,
            poll_rate,
            active_inspector_processing,
            completed_blocks_range,
            total_blocks_range,
            block_tracing_throughput: block_tracing,
            classification_throughput: tree_builder,
            // completed_blocks: metrics::register_counter!("brontes_total_completed_blocks"),
            // processing_run_time_ms:
            // metrics::register_histogram!("brontes_processing_runtime_ms"),
        }
    }

    pub fn add_pending_tree(&self, id: usize) {
        self.pending_trees
            .with_label_values(&[&format!("{id}")])
            .inc();
    }

    pub fn remove_pending_tree(&self, id: usize) {
        self.pending_trees
            .with_label_values(&[&format!("{id}")])
            .dec();
    }

    pub fn inc_inspector(&self, id: usize) {
        self.active_inspector_processing
            .with_label_values(&[&format!("{id}")])
            .inc();
    }

    pub fn dec_inspector(&self, id: usize) {
        self.active_inspector_processing
            .with_label_values(&[&format!("{id}")])
            .dec();
    }

    pub fn poll_rate(&self, id: usize) {
        self.poll_rate.with_label_values(&[&format!("{id}")]).inc();
    }

    pub fn finished_block(&self, id: usize) {
        let strd = format!("{id}");
        self.completed_blocks_range
            .get_metric_with_label_values(&[&strd])
            .unwrap()
            .inc();

        // self.completed_blocks.increment(1);
    }

    pub async fn tree_builder<R>(
        self,
        id: usize,
        f: impl FnOnce() -> Pin<Box<dyn futures::Future<Output = R> + Send>>,
    ) -> R {
        let instant = Instant::now();
        let res = f().await;
        let elapsed = instant.elapsed().as_millis();
        self.classification_throughput
            .with_label_values(&[&format!("{id}")])
            .observe(elapsed as f64);
        res
    }

    pub async fn block_tracing<R>(
        self,
        id: usize,
        f: impl FnOnce() -> Pin<Box<dyn futures::Future<Output = R> + Send>>,
    ) -> R {
        let instant = Instant::now();
        let res = f().await;
        let elapsed = instant.elapsed().as_millis();
        self.block_tracing_throughput
            .with_label_values(&[&format!("{id}")])
            .observe(elapsed as f64);
        res
    }

    pub async fn meter_processing<R>(
        self,
        f: impl FnOnce() -> Pin<Box<dyn futures::Future<Output = R> + Send>>,
    ) -> R {
        // let time = Instant::now();
        let res = f().await;
        // let elapsed = time.elapsed().as_millis() as f64;
        // self.processing_run_time_ms.record(elapsed);

        res
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
