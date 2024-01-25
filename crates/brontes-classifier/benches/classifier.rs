use brontes_classifier::test_utils::ClassifierBenchUtils;
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_tree_building(c: &mut Criterion) {
    let utils = ClassifierBenchUtils::new();
    utils
        .bench_block_tree_building("build 28m gas tree", 18672183, c)
        .unwrap();
}



criterion_group!(tree, bench_tree_building);
criterion_main!(tree);
