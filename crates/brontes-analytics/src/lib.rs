mod builder;
use brontes_database::libmdbx::LibmdbxInit;
use brontes_types::traits::TracingProvider;

pub struct BrontesAnalytics<T: TracingProvider, DB: LibmdbxInit> {
    pub db: &'static DB,
    pub tracing_client: T,
}

impl<T: TracingProvider, DB: LibmdbxInit> BrontesAnalytics<T, DB> {
    pub fn new(db: &'static DB, tracing_client: T) -> Self {
        Self { db, tracing_client }
    }
}
