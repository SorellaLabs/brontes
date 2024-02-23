use alloy_primitives::hex;
use brontes_types::{
    constants::{ETH_ADDRESS, USDC_ADDRESS, WBTC_ADDRESS},
    pair::Pair,
};
use criterion::{criterion_group, criterion_main, Criterion};
use pricing_test_utils::bench::BrontesPricingBencher;

pub fn bench_v3_price_requests(c: &mut Criterion) {
    let bencher = BrontesPricingBencher::new(USDC_ADDRESS);
    bencher
        .bench_pool_state_price(
            "weth usdc pool",
            hex!("9a772018fbd77fcd2d25657e5c547baff3fd7d16").into(),
            18500018,
            Pair(WBTC_ADDRESS, USDC_ADDRESS),
            brontes_types::Protocol::UniswapV3,
            c,
        )
        .unwrap();
}

pub fn bench_v3_state_loads(c: &mut Criterion) {
    let bencher = BrontesPricingBencher::new(USDC_ADDRESS);
    bencher
        .bench_pool_state_loads(
            "weth usdc pool",
            hex!("9a772018fbd77fcd2d25657e5c547baff3fd7d16").into(),
            18500018,
            Pair(WBTC_ADDRESS, USDC_ADDRESS),
            brontes_types::Protocol::UniswapV3,
            c,
        )
        .unwrap();
}

pub fn bench_v2_price_requests(c: &mut Criterion) {
    let bencher = BrontesPricingBencher::new(USDC_ADDRESS);
    bencher
        .bench_pool_state_price(
            "wbtc eth",
            hex!("bb2b8038a1640196fbe3e38816f3e67cba72d940").into(),
            18500018,
            Pair(WBTC_ADDRESS, ETH_ADDRESS),
            brontes_types::Protocol::UniswapV2,
            c,
        )
        .unwrap();
}

pub fn bench_v2_state_loads(c: &mut Criterion) {
    let bencher = BrontesPricingBencher::new(USDC_ADDRESS);
    bencher
        .bench_pool_state_loads(
            "wbtc eth",
            hex!("bb2b8038a1640196fbe3e38816f3e67cba72d940").into(),
            18500018,
            Pair(WBTC_ADDRESS, ETH_ADDRESS),
            brontes_types::Protocol::UniswapV2,
            c,
        )
        .unwrap();
}
