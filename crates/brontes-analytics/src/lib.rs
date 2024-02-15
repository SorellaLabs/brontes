mod builder;

use brontes_database::libmdbx::LibmdbxReadWriter;
use brontes_types::traits::TracingProvider;

pub fn static_object<T>(obj: T) -> &'static T {
    &*Box::leak(Box::new(obj))
}
pub struct BrontesAnalytics<'a, T: TracingProvider> {
    pub libmdbx: &'a LibmdbxReadWriter,
    pub tracing_client: T,
}

impl<'a, T: TracingProvider> BrontesAnalytics<'_, T> {
    pub fn new(libmdbx: &'static LibmdbxReadWriter, tracing_client: T) -> Self {
        Self {
            libmdbx,
            tracing_client,
        }
    }
}
