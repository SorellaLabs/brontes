use database::Database;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use poirot_metrics::PoirotMetricEvents;
pub mod database;

pub struct Labeller {
    client: Database,
    pub(crate) metrics_tx: Arc<UnboundedSender<PoirotMetricEvents>>,
}

impl Labeller {
    pub fn new(metrics_tx: Arc<UnboundedSender<PoirotMetricEvents>>) -> Self {
        Self { client: Database::default(), metrics_tx }
    }
}

