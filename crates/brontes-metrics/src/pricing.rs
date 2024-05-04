use std::time::Instant;

use metrics::{Counter, Gauge, Histogram};
use reth_metrics::Metrics;

#[derive(Metrics, Clone)]
#[metrics(scope = "dex_pricing")]
pub struct DexPricingMetrics {
    /// the amount of active subgraphs currently used for pricing
    pub active_subgraphs:   Gauge,
    /// the amount of active pool state loaded for the subgraphs
    pub active_state:       Gauge,
    /// current state load queries
    pub state_load_queries: Gauge,
    /// state load processing time
    pub state_load_time_us: Histogram,
    /// blocks processed,
    pub processed_blocks:   Counter,
}

impl DexPricingMetrics {
    pub async fn meter_state_load<R>(
        &self,
        f: FnOnce() -> (impl futures::Future<Output = R> + Send),
    ) -> R {
        let time = Instant::now();
        let res = f().await;
        let elapsed = time.elapsed().as_millis();
        self.state_load_time_us.record(elapsed);

        res
    }
}
