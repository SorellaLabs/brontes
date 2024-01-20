use brontes_core::test_utils::TraceLoader;
use thiserror::Error;

pub struct PricingTestUtils {
    tracer: TraceLoader,
}

impl PricingTestUtils {
    pub fn new() -> Self {
        let tracer = TraceLoader::new();
        Self { tracer }
    }
}

#[derive(Debug, Error)]
pub enum PricingTestError {}
