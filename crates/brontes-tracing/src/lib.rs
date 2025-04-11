use std::fmt::Display;

use tracing::Subscriber;
use tracing_subscriber::{
    prelude::__tracing_subscriber_SubscriberExt, registry::LookupSpan, Registry,
};

/// A boxed tracing Layer.
pub type BoxedLayer<S> = Box<dyn Layer<S> + Send + Sync>;

/// Initializes a new [Subscriber] based on the given layers.
pub fn init(layers: Vec<BoxedLayer<Registry>>) {
    let _ = tracing_subscriber::registry().with(layers).try_init();
}

use tracing_subscriber::{layer::Layer, util::SubscriberInitExt, EnvFilter};

pub fn stdout<S>(default_directive: impl Display) -> BoxedLayer<S>
where
    S: Subscriber,
    for<'a> S: LookupSpan<'a>,
{
    let filter = EnvFilter::builder()
        .with_default_directive(default_directive.to_string().parse().unwrap())
        .from_env_lossy()
        .add_directive("hyper::proto::h1=off".parse().unwrap())
        .add_directive("providers::static_file=off".parse().unwrap());

    tracing_subscriber::fmt::layer()
        .with_ansi(true)
        .with_target(true)
        .with_filter(filter)
        .boxed()
}
