use brontes_types::constants::USDC_ADDRESS;
use criterion::{criterion_group, criterion_main, Criterion};
use pricing_test_utils::bench::BrontesPricingBencher;

pub fn bench_block_pricing(c: &mut Criterion) {
    let bencher = BrontesPricingBencher::new(USDC_ADDRESS);
    bencher
        .bench_pricing_block("block 18500018", 18500018, c)
        .unwrap();
}
