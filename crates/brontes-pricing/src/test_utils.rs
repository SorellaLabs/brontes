use brontes::core_test_utils::TraceLoader;


pub struct PricingTestUtils {
    tracer: TraceLoader,
}

impl PricingTestUtils {
    pub fn new() -> Self {
        let tracer = TraceLoader::new();
        Self { tracer }
    }
}
