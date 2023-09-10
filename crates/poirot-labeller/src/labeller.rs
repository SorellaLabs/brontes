
#[derive(Clone)]
/// A [`TraceParser`] will iterate through a block's Parity traces and attempt to decode each call
/// for later analysis.
pub struct Labeller {
    client: ClickhouseClient,
    pub(crate) metrics_tx: Arc<UnboundedSender<PoirotMetricEvents>>,
}

