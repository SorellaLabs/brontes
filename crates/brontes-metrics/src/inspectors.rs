use std::{collections::HashSet, pin::Pin, time::Instant};

use brontes_types::{mev::MevType, pair::Pair, FastHashMap, Protocol};
use dashmap::DashMap;
use metrics::{Counter, Gauge};
use prometheus::{HistogramVec, IntCounterVec};
use reth_metrics::Metrics;
use reth_primitives::Address;

#[derive(Clone)]
pub struct ProfitMetrics {
    profit_histogram: HistogramVec,
    timeboost_profit_histogram: HistogramVec,
}

impl Default for ProfitMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl ProfitMetrics {
    pub fn new() -> Self {
        // Define custom buckets for profit values (e.g., in USD)
        let profit_buckets = prometheus::exponential_buckets(0.1, 2.0, 15).unwrap_or_else(|_| {
            vec![1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0, 10000.0]
        }); // Example buckets

        Self {
            profit_histogram: prometheus::register_histogram_vec!(
                "profit_usd",
                "Distribution of profit in USD by MEV type, protocol, and block_number",
                &["mev_type", "protocol"],
                profit_buckets.clone(),
            )
            .expect("Failed to register profit_usd histogram"),
            timeboost_profit_histogram: prometheus::register_histogram_vec!(
                "timeboost_profit_usd",
                "Distribution of timeboosted tx profit in USD by MEV type, protocol, and block_number",
                &["mev_type", "protocol"],
                profit_buckets,
            )
            .expect("Failed to register timeboost_profit_usd histogram"),
        }
    }

    pub fn publish_profit_metrics(&self, mev: MevType, protocols: HashSet<Protocol>, profit: f64, timeboosted: bool) {
        let num_protocols = protocols.len();
        let profit_per_protocol = profit / num_protocols as f64;
        for protocol in protocols {
            self.profit_histogram
                .with_label_values(&[mev.as_ref(), protocol.to_string().as_str()])
                .observe(profit_per_protocol);
            if timeboosted {
                self.timeboost_profit_histogram
                    .with_label_values(&[mev.as_ref(), protocol.to_string().as_str()])
                    .observe(profit_per_protocol);
            }
        }
    }
}

impl std::fmt::Debug for ProfitMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProfitMetrics").finish()
    }
}

#[derive(Clone)]
pub struct OutlierMetrics {
    // missed data
    pub cex_pair_symbols:          IntCounterVec,
    // missed data
    pub dex_bad_pricing:           IntCounterVec,
    pub inspector_100x_price_type: IntCounterVec,
    pub branch_filtering_trigger:  IntCounterVec,
    // runtimes
    inspector_runtime:             HistogramVec,
    cex_dex_price_speed:           HistogramVec,
}

impl Default for OutlierMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl OutlierMetrics {
    pub fn new() -> Self {
        let cex_pair_symbols = prometheus::register_int_counter_vec!(
            "cex_pair_no_symbols",
            "the count of cex dex missed due to no cex symbol to address",
            &["token0", "token1"]
        )
        .unwrap();

        let dex_bad_pricing = prometheus::register_int_counter_vec!(
            "brontes_bad_dex_pricing",
            "the amount of arbs filtered out due to inncorrect pricing",
            &["mev_type", "token0", "token1"]
        )
        .unwrap();

        let inspector_100x_price_type = prometheus::register_int_counter_vec!(
            "brontes_100x_profit",
            "the amount of arbs that exceed 100x profit ratio",
            &["mev_type"]
        )
        .unwrap();

        let branch_filtering_trigger = prometheus::register_int_counter_vec!(
            "brontes_branch_filtering_trigger",
            "the branch that caused the mev to be filtered out",
            &["mev_type", "branch_name"]
        )
        .unwrap();

        let buckets = prometheus::exponential_buckets(1.0, 2.0, 22).unwrap();

        let inspector_runtime = prometheus::register_histogram_vec!(
            "inspector_runtime_ms",
            "the runtime of the inspectors",
            &["mev_type"],
            buckets.clone()
        )
        .unwrap();

        let cex_dex_price_speed = prometheus::register_histogram_vec!(
            "cex_dex_price_speed",
            "the runtime of the inspectors",
            &["type"],
            buckets
        )
        .unwrap();

        Self {
            inspector_runtime,
            branch_filtering_trigger,
            inspector_100x_price_type,
            dex_bad_pricing,
            cex_pair_symbols,
            cex_dex_price_speed,
        }
    }

    pub fn run_cex_price_window<R>(&self, f: impl FnOnce() -> Option<R>) -> Option<R> {
        let instant = Instant::now();
        let res = f();
        let elapsed = instant.elapsed().as_millis();
        if res.is_some() {
            self.cex_dex_price_speed
                .with_label_values(&["window"])
                .observe(elapsed as f64);
        }
        res
    }

    pub fn run_cex_price_vol<R>(&self, f: impl FnOnce() -> Option<R>) -> Option<R> {
        let instant = Instant::now();
        let res = f();
        let elapsed = instant.elapsed().as_millis();

        if res.is_some() {
            self.cex_dex_price_speed
                .with_label_values(&["volume"])
                .observe(elapsed as f64);
        }

        res
    }

    pub fn run_inspector<R>(&self, inspector_type: MevType, f: impl FnOnce() -> R) -> R {
        let instant = Instant::now();
        let res = f();
        let elapsed = instant.elapsed().as_millis();

        self.inspector_runtime
            .with_label_values(&[inspector_type.as_ref()])
            .observe(elapsed as f64);
        res
    }

    pub fn missing_cex_pair(&self, pair: Pair) {
        let pair = pair.ordered();
        let t0 = format!("{:?}", pair.0);
        let t1 = format!("{:?}", pair.1);
        self.cex_pair_symbols
            .get_metric_with_label_values(&[&t0, &t1])
            .unwrap()
            .inc()
    }

    pub fn bad_dex_pricing(&self, mev: MevType, pair: Pair) {
        let pair = pair.ordered();
        let t0 = format!("{:?}", pair.0);
        let t1 = format!("{:?}", pair.1);

        let t = mev.to_string();
        self.dex_bad_pricing
            .get_metric_with_label_values(&[&t, &t0, &t1])
            .unwrap()
            .inc();
    }

    pub fn inspector_100x_profit(&self, mev_type: MevType) {
        let t = mev_type.to_string();
        self.inspector_100x_price_type
            .get_metric_with_label_values(&[&t])
            .unwrap()
            .inc();
    }

    pub fn branch_filtering_trigger(&self, mev_type: MevType, branch_name: &'static str) {
        let t = mev_type.to_string();

        self.branch_filtering_trigger
            .get_metric_with_label_values(&[&t, branch_name])
            .unwrap()
            .inc();
    }
}

impl std::fmt::Debug for OutlierMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OutlierMetrics").finish()
    }
}
