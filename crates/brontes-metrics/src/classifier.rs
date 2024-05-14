use std::{pin::Pin, time::Instant};

use alloy_primitives::Address;
use brontes_types::Protocol;
use dashmap::DashMap;
use metrics::{Counter, Gauge, Histogram};
use prometheus::IntCounterVec;

#[derive(Clone)]
pub struct ClassificationMetrics {
    pub bad_protocol_classification: IntCounterVec,
}

impl Default for ClassificationMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl ClassificationMetrics {
    pub fn new() -> Self {
        let bad_protocol_classification = prometheus::register_int_counter_vec!(
            "brontes_bad_protocol_classification",
            "when we have a classification error",
            &["protocol"]
        )
        .unwrap();
        Self { bad_protocol_classification }
    }

    pub fn bad_protocol_classification(&self, protocol: Protocol) {
        self.bad_protocol_classification
            .get_metric_with_label_values(&[&protocol.to_string()])
            .unwrap()
            .inc()
    }
}
