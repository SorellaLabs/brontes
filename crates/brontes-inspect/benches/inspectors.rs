use std::str::FromStr;

use alloy_primitives::{hex, B256};
use brontes_classifier::test_utils::ClassifierTestUtils;
use brontes_inspect::{
    test_utils::{InspectorBenchUtils, USDC_ADDRESS},
    Inspectors,
};
use criterion::{criterion_group, criterion_main, Criterion};
use itertools::Itertools;
use strum::IntoEnumIterator;

fn bench_sandwich(c: &mut Criterion) {
    let bencher = InspectorBenchUtils::new(USDC_ADDRESS);
    bencher
        .bench_inspector_txes(
            "regular sandwich",
            vec![
                hex!("849c3cb1f299fa181e12b0506166e4aa221fce4384a710ac0d2e064c9b4e1c42").into(),
                hex!("055f8dd4eb02c15c1c1faa9b65da5521eaaff54f332e0fa311bc6ce6a4149d18").into(),
                hex!("ab765f128ae604fdf245c78c8d0539a85f0cf5dc7f83a2756890dea670138506").into(),
                hex!("06424e50ee53df1e06fa80a741d1549224e276aed08c3674b65eac9e97a39c45").into(),
                hex!("c0422b6abac94d29bc2a752aa26f406234d45e4f52256587be46255f7b861893").into(),
            ],
            0,
            Inspectors::Sandwich,
            vec![],
            c,
        )
        .unwrap()
}

fn bench_sandwich_big_mac(c: &mut Criterion) {
    let bencher = InspectorBenchUtils::new(USDC_ADDRESS);
    bencher
        .bench_inspector_txes(
            "big mac sandwich",
            vec![
                hex!("2a187ed5ba38cc3b857726df51ce99ee6e29c9bcaa02be1a328f99c3783b3303").into(),
                hex!("7325392f41338440f045cb1dba75b6099f01f8b00983e33cc926eb27aacd7e2d").into(),
                hex!("bcb8115fb54b7d6b0a0b0faf6e65fae02066705bd4afde70c780d4251a771428").into(),
                hex!("0b428553bc2ccc8047b0da46e6c1c1e8a338d9a461850fcd67ddb233f6984677").into(),
                hex!("fb2ef488bf7b6ad09accb126330837198b0857d2ea0052795af520d470eb5e1d").into(),
            ],
            0,
            Inspectors::Sandwich,
            vec![],
            c,
        )
        .unwrap()
}

fn bench_backrun_triagular(c: &mut Criterion) {
    let bencher = InspectorBenchUtils::new(USDC_ADDRESS);
    bencher
        .bench_inspector_txes(
            "backrun triagular",
            vec![hex!("67d9884157d495df4eaf24b0d65aeca38e1b5aeb79200d030e3bb4bd2cbdcf88").into()],
            0,
            Inspectors::AtomicArb,
            vec![],
            c,
        )
        .unwrap()
}
fn bench_backrun_10_swaps(c: &mut Criterion) {
    let bencher = InspectorBenchUtils::new(USDC_ADDRESS);
    bencher
        .bench_inspector_txes(
            "bench backrun 10 swaps",
            vec![hex!("76971a4f00a0a836322c9825b6edf06c8c49bf4261ef86fc88893154283a7124").into()],
            0,
            Inspectors::AtomicArb,
            vec![],
            c,
        )
        .unwrap()
}

fn bench_liquidation(c: &mut Criterion) {
    let bencher = InspectorBenchUtils::new(USDC_ADDRESS);
    bencher
        .bench_inspector_txes(
            "bench aave v2 bench_liquidation",
            vec![hex!("725551f77f94f0ff01046aa4f4b93669d689f7eda6bb8cd87e2be780935eb2db").into()],
            0,
            Inspectors::Liquidations,
            vec![],
            c,
        )
        .unwrap()
}

fn bench_cex_dex(c: &mut Criterion) {
    let rt = tokio::runtime::Handle::current();
    let tx_hash =
        B256::from_str("0x21b129d221a4f169de0fc391fe0382dbde797b69300a9a68143487c54d620295")
            .unwrap();

    let classifer_utils = rt.block_on(ClassifierTestUtils::new());
    let metadata = rt.block_on(classifer_utils.get_metadata(0, true)).unwrap();

    let bencher = InspectorBenchUtils::new(USDC_ADDRESS);
    bencher
        .bench_inspector_txes_with_meta(
            "bench cex dex, 100 per iter",
            vec![tx_hash],
            metadata,
            100,
            Inspectors::CexDex,
            c,
        )
        .unwrap()
}

fn bench_composer(c: &mut Criterion) {
    let bencher = InspectorBenchUtils::new(USDC_ADDRESS);
    bencher
        .bench_composer(
            "bench sandwich jit composer",
            vec![
                hex!("22ea36d516f59cc90ccc01042e20f8fba196f32b067a7e5f1510099140ae5e0a").into(),
                hex!("72eb3269ac013cf663dde9aa11cc3295e0dfb50c7edfcf074c5c57b43611439c").into(),
                hex!("3b4138bac9dc9fa4e39d8d14c6ecd7ec0144fe26b120ea799317aa15fa35ddcd").into(),
                hex!("99785f7b76a9347f13591db3574506e9f718060229db2826b4925929ebaea77e").into(),
                hex!("31dedbae6a8e44ec25f660b3cd0e04524c6476a0431ab610bb4096f82271831b").into(),
            ],
            0,
            vec![Inspectors::Sandwich, Inspectors::Jit],
            vec![],
            c,
        )
        .unwrap()
}

fn bench_regular_block(c: &mut Criterion) {
    let bencher = InspectorBenchUtils::new(USDC_ADDRESS);
    bencher
        .bench_composer_block(
            "bench block 28mill gas",
            18672183,
            0,
            Inspectors::iter().collect_vec(),
            vec![],
            c,
        )
        .unwrap()
}

fn bench_sandwich_regular_block(c: &mut Criterion) {
    let bencher = InspectorBenchUtils::new(USDC_ADDRESS);
    bencher
        .bench_inspectors_block(
            "bench sandwich 12mil gas",
            18500002,
            0,
            vec![Inspectors::Sandwich],
            vec![],
            c,
        )
        .unwrap()
}

fn bench_liquidations_regular_block(c: &mut Criterion) {
    let bencher = InspectorBenchUtils::new(USDC_ADDRESS);
    bencher
        .bench_inspectors_block(
            "aave v2 liquidation 14 mill gas block",
            18979710,
            0,
            vec![Inspectors::Liquidations],
            vec![],
            c,
        )
        .unwrap()
}

fn bench_backrun_regular_block(c: &mut Criterion) {
    let bencher = InspectorBenchUtils::new(USDC_ADDRESS);
    bencher
        .bench_inspectors_block(
            "backrun 15 mill gas block",
            18000103,
            0,
            vec![Inspectors::AtomicArb],
            vec![],
            c,
        )
        .unwrap()
}

fn bench_jit_regular_block(c: &mut Criterion) {
    let bencher = InspectorBenchUtils::new(USDC_ADDRESS);
    bencher
        .bench_inspectors_block(
            "jit 16 mill gas block",
            18500009,
            0,
            vec![Inspectors::Jit],
            vec![],
            c,
        )
        .unwrap()
}

fn bench_cex_dex_regular_block(c: &mut Criterion) {
    let bencher = InspectorBenchUtils::new(USDC_ADDRESS);
    bencher
        .bench_inspectors_block(
            "cex dex 16 mill gas block",
            18264694,
            0,
            vec![Inspectors::CexDex],
            vec![],
            c,
        )
        .unwrap()
}

criterion_group!(
    inspector_specific_tx_benches,
    bench_sandwich,
    bench_sandwich_big_mac,
    bench_backrun_triagular,
    bench_backrun_10_swaps,
    bench_liquidation,
    bench_cex_dex,
    bench_composer,
);

criterion_group!(
    inspector_full_block_benches,
    bench_regular_block,
    bench_sandwich_regular_block,
    bench_liquidations_regular_block,
    bench_backrun_regular_block,
    bench_jit_regular_block,
    bench_cex_dex_regular_block
);

criterion_main!(inspector_specific_tx_benches, inspector_full_block_benches);
