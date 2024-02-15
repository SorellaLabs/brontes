mod builder;
use std::env;

use brontes_database::libmdbx::{Libmdbx, LibmdbxReadWriter, LibmdbxReader, LibmdbxWriter};
use reth_tracing_ext::TracingClient;

pub fn static_object<T>(obj: T) -> &'static T {
    &*Box::leak(Box::new(obj))
}
pub struct BrontesAnalytics<'a, T: TracingProvider> {
    pub libmdbx: &'static LibmdbxReadWriter,
    pub tracing_client: T,
}

impl<'a, T: TracingProvider> BrontesAnalytics {
    pub fn new(libmdbx: &'static LibmdbxReadWriter, tracing_client: T) -> Self {
        Self {
            libmdbx,
            tracing_client,
        }
    }
}
