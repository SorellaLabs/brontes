use crate::trace::{types::TraceMetricEvent, TraceMetrics};
use dyn_contracts::{types::DynamicContractMetricEvent, DynamicContractMetrics};
use futures::Future;
use std::{
    collections::HashMap,
    pin::Pin,
    task::{ready, Context, Poll},
};
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::trace;
pub mod dyn_contracts;
pub mod prometheus_exporter;
pub mod trace;

/// metric event for traces
#[derive(Clone, Debug)]
pub enum PoirotMetricEvents {
    /// recorded a new trace event
    TraceMetricRecieved(TraceMetricEvent),
    /// recorded a new dynamic contract recording
    DynamicContractMetricRecieved(DynamicContractMetricEvent),
}

/// Metrics routine that listens to new metric events on the `events_rx` receiver.
/// Upon receiving new event, related metrics are updated.
#[derive(Debug)]
pub struct PoirotMetricsListener {
    events_rx: UnboundedReceiver<PoirotMetricEvents>,
    tx_metrics: TraceMetrics,
    contract_metrics: HashMap<String, DynamicContractMetrics>,
}

impl PoirotMetricsListener {
    /// Creates a new [MetricsListener] with the provided receiver of [MetricEvent].
    pub fn new(events_rx: UnboundedReceiver<PoirotMetricEvents>) -> Self {
        Self {
            events_rx,
            tx_metrics: TraceMetrics::default(),
            contract_metrics: HashMap::default(),
        }
    }

    fn handle_event(&mut self, event: PoirotMetricEvents) {
        trace!(target: "tracing::metrics", ?event, "Metric event received");
        match event {
            PoirotMetricEvents::TraceMetricRecieved(val) => self.tx_metrics.handle_event(val),
            PoirotMetricEvents::DynamicContractMetricRecieved(val) => {
                let this = self
                    .contract_metrics
                    .entry(val.get_addr())
                    .or_default();
                this.handle_event(val)
            }
        }
    }
}

impl Future for PoirotMetricsListener {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        loop {
            let Some(event) = ready!(this.events_rx.poll_recv(cx)) else { return Poll::Ready(()) };

            this.handle_event(event);
        }
    }
}
