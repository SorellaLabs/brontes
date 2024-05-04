use std::{pin::Pin, time::Instant};

use alloy_primitives::Address;
use brontes_types::Protocol;
use dashmap::DashMap;
use metrics::{Counter, Gauge, Histogram};

#[derive(Clone, Default)]
pub struct ClassificationMetrics {
    pub bad_protocol_classification: DashMap<Protocol, Counter>,
}
impl ClassificationMetrics {
    pub fn bad_protocol_classification(&self, protocol: Protocol) {
        self.bad_protocol_classification
            .entry(protocol)
            .or_insert_with(|| {
                metrics::register_counter!(format!("{protocol}_failed_classification"))
            })
            .increment(1);
    }
}
