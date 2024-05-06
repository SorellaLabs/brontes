use std::{pin::Pin, time::Instant};

use brontes_types::{mev::MevType, pair::Pair, FastHashMap};
use dashmap::DashMap;
use metrics::{Counter, Gauge, Histogram};
use prometheus::IntCounterVec;
use reth_metrics::Metrics;
use reth_primitives::Address;

#[derive(Clone)]
pub struct OutlierMetrics {
    // missed data
    pub cex_pair_symbols:          IntCounterVec,
    // missed data
    pub dex_bad_pricing:           IntCounterVec,
    pub inspector_100x_price_type: IntCounterVec,
    pub branch_filtering_trigger:  IntCounterVec,
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

        Self {
            branch_filtering_trigger,
            inspector_100x_price_type,
            dex_bad_pricing,
            cex_pair_symbols,
        }
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
