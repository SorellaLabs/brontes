use tracing::Subscriber;
use tracing_subscriber::{*, filter::Directive, registry::LookupSpan, prelude::*};


/// A boxed tracing Layer.
pub type BoxedLayer<S> = Box<dyn Layer<S> + Send + Sync>;


/// Initializes a new [Subscriber] based on the given layers.
pub fn init(layers: Vec<BoxedLayer<Registry>>) {
    tracing_subscriber::registry().with(layers).init();
}


/// Builds a new tracing layer that writes to stdout.
pub fn stdout<S>(directive: impl Into<Directive>) -> BoxedLayer<S>
where
    S: Subscriber,
    for<'a> S: LookupSpan<'a>,
{
    let filter = EnvFilter::builder().with_default_directive(directive.into()).from_env_lossy();

    tracing_subscriber::fmt::layer()
        .with_ansi(true)
        .with_target(true)
        .with_filter(filter)
        .boxed()
}
