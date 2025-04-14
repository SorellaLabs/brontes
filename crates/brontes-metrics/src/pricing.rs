use std::{fmt::Debug, pin::Pin, time::Instant};

use metrics::{Counter, Gauge, Histogram};
use prometheus::{IntCounterVec, IntGaugeVec};
use reth_metrics::Metrics;

#[derive(Clone)]
pub struct DexPricingMetrics {
    // /// the amount of active subgraphs currently used for pricing
    // pub active_subgraphs:    Gauge,
    // /// the amount of active pool state loaded for the subgraphs
    // pub active_state:        Gauge,
    // /// current state load queries
    // pub state_load_queries:  Gauge,
    // /// state load processing time
    // pub state_load_time_ms:  Histogram,
    // /// blocks processed,
    // pub processed_blocks:    Counter,
    // /// block processing speed by range
    // pub range_processing:    IntCounterVec,
    // /// function call count
    // pub function_call_count: IntCounterVec,
    // /// rate of poll
    // pub poll_rate:           IntCounterVec,
    // /// wants more blocks
    // pub needs_more_data:     IntGaugeVec,
}
impl Default for DexPricingMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Debug for DexPricingMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DexPricingMetrics").finish()
    }
}

impl DexPricingMetrics {
    pub fn new() -> Self {
        // let active_subgraphs = metrics::register_gauge!("dex_pricing_active_subgraphs");
        // let active_state = metrics::register_gauge!("dex_pricing_active_state");
        // let state_load_queries = metrics::register_gauge!("dex_pricing_state_load_queries");
        // let state_load_time_ms = metrics::register_histogram!("dex_pricing_state_load_time_ms");
        // let processed_blocks = metrics::register_counter!("dex_pricing_processed_blocks");
        // let range_processing = prometheus::register_int_counter_vec!(
        //     "dex_pricing_range_processed_blocks",
        //     "the amount of blocks a range has processed",
        //     &["range_id"]
        // )
        // .unwrap();
        // let function_call_count = prometheus::register_int_counter_vec!(
        //     "dex_pricing_function_call_count",
        //     "for each range and function name the call count",
        //     &["range_id", "function_name"]
        // )
        // .unwrap();

        // let poll_rate = prometheus::register_int_counter_vec!(
        //     "dex_pricing_range_poll_rate",
        //     "the poll rate for the future of the range",
        //     &["range_id"]
        // )
        // .unwrap();

        // let needs_more_data = prometheus::register_int_gauge_vec!(
        //     "dex_pricing_needs_more_data",
        //     "wether or not dex pricing is asking for more data",
        //     &["range_id"]
        // )
        // .unwrap();

        Self {
            // needs_more_data,
            // processed_blocks,
            // state_load_time_ms,
            // state_load_queries,
            // active_state,
            // active_subgraphs,
            // range_processing,
            // function_call_count,
            // poll_rate,
        }
    }

    pub fn needs_more_data(&self, range_id: usize, enabled: bool) {
        // self.needs_more_data
        //     .with_label_values(&[&range_id.to_string()])
        //     .set(enabled as i64);
    }

    pub fn poll_rate(&self, range_id: usize) {
        // self.poll_rate
        //     .get_metric_with_label_values(&[&range_id.to_string()])
        //     .unwrap()
        //     .inc();
    }

    pub fn function_call_count(&self, range_id: usize, function_name: &str) {
        // self.function_call_count
        //     .with_label_values(&[&range_id.to_string(), function_name])
        //     .inc()
    }

    pub fn range_finished_block(&self, range_id: usize) {
        // self.processed_blocks.increment(1);
        // self.range_processing
        //     .get_metric_with_label_values(&[&range_id.to_string()])
        //     .unwrap()
        //     .inc();
    }

    pub async fn meter_state_load<R>(
        self,
        f: impl FnOnce() -> Pin<Box<dyn futures::Future<Output = R> + Send>>,
    ) -> R {
        // self.state_load_queries.increment(1.0);
        // let time = Instant::now();
        let res = f().await;
        // let elapsed = time.elapsed().as_millis() as f64;
        // self.state_load_time_ms.record(elapsed);
        // self.state_load_queries.decrement(1.0);

        res
    }
}
