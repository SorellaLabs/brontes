#![allow(unused)]
use std::{
    collections::HashMap,
    pin::Pin,
    task::{Context, Poll},
};

use brontes_types::UnboundedYapperReceiver;
use db_interfaces::errors::DatabaseError;
use dyn_contracts::{types::DynamicContractMetricEvent, DynamicContractMetrics};
use futures::Future;
use tracing::trace;

use crate::trace::{types::TraceMetricEvent, TraceMetrics};
pub mod classifier;
pub mod db_cache;
pub mod db_initialization;
pub mod db_reads;
pub mod db_writer;
pub mod dyn_contracts;
pub mod error_layer;
pub mod inspectors;
pub mod pricing;
pub mod prometheus_exporter;
pub mod range;
pub mod trace;

/// metric event for traces
#[derive(Clone, Debug)]
pub enum ParserMetricEvents {
    /// recorded a new trace event
    TraceMetricRecieved(TraceMetricEvent),
    /// recorded a new dynamic contract recording
    DynamicContractMetricRecieved(DynamicContractMetricEvent),
}

/// Metrics routine that listens to new metric events on the `events_rx`
/// receiver. Upon receiving new event, related metrics are updated.
pub struct ParserMetricsListener {
    events_rx:        UnboundedYapperReceiver<ParserMetricEvents>,
    tx_metrics:       TraceMetrics,
    contract_metrics: HashMap<String, DynamicContractMetrics>,
}

impl ParserMetricsListener {
    /// Creates a new `MetricsListener` with the provided receiver of
    /// MetricEvent.
    pub fn new(events_rx: UnboundedYapperReceiver<ParserMetricEvents>) -> Self {
        Self {
            events_rx,
            tx_metrics: TraceMetrics::default(),
            contract_metrics: HashMap::default(),
        }
    }

    fn handle_event(&mut self, event: ParserMetricEvents) {
        trace!(target: "tracing::metrics", ?event, "Metric event received");
        match event {
            ParserMetricEvents::TraceMetricRecieved(val) => self.tx_metrics.handle_event(val),
            ParserMetricEvents::DynamicContractMetricRecieved(val) => {
                let this = self.contract_metrics.entry(val.get_addr()).or_default();
                this.handle_event(val)
            }
        }
    }
}

impl Future for ParserMetricsListener {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        while let Poll::Ready(Some(event)) = this.events_rx.poll_recv(cx) {
            drop(event);
            // this.handle_event(event);
        }

        cx.waker().wake_by_ref();
        Poll::Pending
    }
}
