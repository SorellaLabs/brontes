use dashmap::DashMap;
use metrics::Counter;
use tracing::{Level, Subscriber};
use tracing_subscriber::Layer;

#[derive(Clone)]
pub struct BrontesErrorMetrics {
    error_count: prometheus::IntCounterVec,
}
impl BrontesErrorMetrics {
    pub fn new() -> Self {
        let error_count = prometheus::register_int_counter_vec!(
            "brontes_log_count_with_target",
            "the amount of logs per target with level",
            &["level", "target"]
        )
        .unwrap();
        Self { error_count }
    }
}

impl Default for BrontesErrorMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: Subscriber> Layer<S> for BrontesErrorMetrics {
    fn on_event(&self, event: &tracing::Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        if event.metadata().level().eq(&Level::INFO)
            || event.metadata().level().eq(&Level::WARN)
            || event.metadata().level().eq(&Level::ERROR)
        {
            let level = event.metadata().level();
            let target = event.metadata().target();

            self.error_count
                .with_label_values(&[&level.to_string(), target])
                .inc()
        }
    }
}
