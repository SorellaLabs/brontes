use dashmap::DashMap;
use metrics::Counter;
use tracing::{Level, Subscriber};
use tracing_subscriber::Layer;

#[derive(Clone, Default)]
pub struct BrontesErrorMetrics {
    error_count: DashMap<&'static str, Counter>,
}

impl<S: Subscriber> Layer<S> for BrontesErrorMetrics {
    fn on_event(&self, event: &tracing::Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        if Level::ERROR.eq(event.metadata().level()) {
            let target = event.metadata().target();
            self.error_count
                .entry(target)
                .or_insert_with(|| metrics::register_counter!(format!("{target}_errors")))
                .increment(1);
        }
    }
}
