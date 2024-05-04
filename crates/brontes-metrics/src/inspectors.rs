use std::{pin::Pin, time::Instant};

use brontes_types::{mev::MevType, FastHashMap};
use metrics::{Counter, Gauge, Histogram};
use reth_metrics::Metrics;
use reth_primitives::Address;

#[derive(Clone)]
pub struct OutlierMetrics {
    // missed data
    pub cex_token_symbols:      FastHashMap<Address, Counter>,
    pub cex_token_symbol_block: FastHashMap<Address, FastHashMap<u64, Counter>>,

    // missed data
    pub dex_bad_pricing:        FastHashMap<Address, Counter>,
    pub dex_token_symbol_block: FastHashMap<Address, FastHashMap<u64, Counter>>,

    /// 100x profit
    pub inspector_100x_price_type: FastHashMap<MevType, Counter>,

    pub branch_filtering_trigger: FastHashMap<MevType, FastHashMap<String, Counter>>,
}

impl OutlierMetrics {}
