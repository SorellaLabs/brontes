mod builder;
use std::env;

use brontes_database::libmdbx::{Libmdbx, LibmdbxReadWriter, LibmdbxReader, LibmdbxWriter};
use reth_tracing_ext::TracingClient;

pub fn static_object<T>(obj: T) -> &'static T {
    &*Box::leak(Box::new(obj))
}
pub struct BrontesAnalytics {
    pub libmdbx: &'static LibmdbxReadWriter,
    pub tracing_client: &'static TracingClient,
}

impl BrontesAnalytics {
    pub fn new(
        libmdbx: &'static LibmdbxReadWriter,
        tracing_client: &'static TracingClient,
    ) -> Self {
        Self {
            libmdbx,
            tracing_client,
        }
    }
}
