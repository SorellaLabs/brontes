use brontes_types::constants::USDC_ADDRESS;
use criterion::Criterion;
use pricing_test_utils::bench::BrontesPricingBencher;

pub fn bench_block_pricing(c: &mut Criterion) {
    let bencher = BrontesPricingBencher::new(USDC_ADDRESS);
    let r = bencher.bench_pricing_block("block 18500018", 18500018, c);
    tracing::info!(err=?r, "result");
    r.unwrap();
}

pub fn bench_block_pricing_after_5_blocks(c: &mut Criterion) {
    let bencher = BrontesPricingBencher::new(USDC_ADDRESS);
    bencher
        .bench_pricing_post_init("pricing after 5 blocks, start = 18500018", 18500018, 5, c)
        .unwrap();
}

pub fn bench_block_pricing_after_10_blocks(c: &mut Criterion) {
    let bencher = BrontesPricingBencher::new(USDC_ADDRESS);
    bencher
        .bench_pricing_post_init("pricing after 10 blocks, start = 18500018", 18500018, 10, c)
        .unwrap();
}

pub fn bench_block_pricing_after_20_blocks(c: &mut Criterion) {
    let bencher = BrontesPricingBencher::new(USDC_ADDRESS);
    bencher
        .bench_pricing_post_init("pricing after 20 blocks, start = 18500018", 18500018, 20, c)
        .unwrap();
}
