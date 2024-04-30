use std::str::FromStr;

use alloy_primitives::B256;
use brontes_classifier::test_utils::ClassifierBenchUtils;
use brontes_types::{normalized_actions::Action, TreeSearchBuilder};
use criterion::{criterion_group, Criterion};

fn bench_collect_tx(c: &mut Criterion) {
    let utils = ClassifierBenchUtils::new();
    utils
        .bench_tree_operations_tx(
            "bench collect tx",
            B256::from_str("0x725551f77f94f0ff01046aa4f4b93669d689f7eda6bb8cd87e2be780935eb2db")
                .unwrap(),
            c,
            |tree| {
                tree.collect_all(TreeSearchBuilder::default().with_action(Action::is_transfer));
            },
        )
        .unwrap();
}

fn bench_collect_block(c: &mut Criterion) {
    let utils = ClassifierBenchUtils::new();
    utils
        .bench_tree_operations("collect block", 18672183, c, |tree| {
            tree.collect_all(TreeSearchBuilder::default().with_action(Action::is_transfer));
        })
        .unwrap();
}

criterion_group!(tree_operations, bench_collect_tx, bench_collect_block);
