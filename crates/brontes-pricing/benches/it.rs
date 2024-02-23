use brontes_types::constants::USDC_ADDRESS;
use criterion::{criterion_group, criterion_main, Criterion};

use crate::test_utils::BrontesPricingBencher;

pub fn bench_block_pricing(c: &mut Criterion) {
    let bencher = BrontesPricingBencher::new(USDC_ADDRESS);
    bencher
        .bench_pricing_block("block 18500018", 18500018, c)
        .unwrap();
}

criterion_group!(it_runs, bench_block_pricing);

criterion_main!(it_runs);
