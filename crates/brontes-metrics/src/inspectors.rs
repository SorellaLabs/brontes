use std::{pin::Pin, time::Instant};

use brontes_types::{mev::MevType, FastHashMap};
use dashmap::DashMap;
use metrics::{Counter, Gauge, Histogram};
use reth_metrics::Metrics;
use reth_primitives::Address;

#[derive(Clone, Default)]
pub struct OutlierMetrics {
    // missed data
    pub cex_token_symbols:         DashMap<Address, Counter>,
    // missed data
    pub dex_bad_pricing:           DashMap<Address, Counter>,
    pub inspector_100x_price_type: DashMap<MevType, Counter>,

    pub branch_filtering_trigger: DashMap<MevType, DashMap<String, Counter>>,
}

impl OutlierMetrics {
    pub fn missing_cex_token(&self, addr: Address) {
        self.cex_token_symbols
            .entry(addr)
            .or_insert_with(|| metrics::register_counter!(format!("{addr:?}_cex_symbol_missing")))
            .increment(1);
    }

    pub fn bad_dex_pricing(&self, addr: Address) {
        self.dex_bad_pricing
            .entry(addr)
            .or_insert_with(|| metrics::register_counter!(format!("{addr:?}_dex_bad_pricing")))
            .increment(1);
    }

    pub fn inspector_100x_profit(&self, mev_type: MevType) {
        self.inspector_100x_price_type
            .entry(mev_type)
            .or_insert_with(|| metrics::register_counter!(format!("{mev_type}_100x_profit")))
            .increment(1);
    }

    pub fn branch_filtering_trigger(&self, mev_type: MevType, branch_name: String) {
        self.branch_filtering_trigger
            .entry(mev_type)
            .or_default()
            .entry(branch_name.clone())
            .or_insert_with(|| {
                metrics::register_counter!(format!("{mev_type}_{branch_name}_filtering"))
            })
            .increment(1);
    }
}

impl std::fmt::Debug for OutlierMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OutlierMetrics").finish()
    }
}
