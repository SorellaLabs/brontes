use std::collections::HashSet;

use brontes_classifier::{
    test_utils::{build_raw_test_tree, get_traces_with_meta},
    Classifier,
};
use brontes_core::{decoding::parser::TraceParser, init_tracing, test_utils::init_trace_parser};
use brontes_database::{database::Database, Metadata};
use brontes_types::{
    normalized_actions::Actions, structured_trace::TxTrace, test_utils::force_call_action,
    tree::Node,
};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use reth_primitives::Address;
use reth_rpc_types::{
    trace::parity::{TraceType, TransactionTrace},
    Header,
};
use reth_tracing::TracingClient;
use serial_test::serial;
use tokio::sync::mpsc::unbounded_channel;

pub async fn setup_data(block_number: u64) -> (Vec<TxTrace>, Header, Metadata) {
    init_tracing();

    dotenv::dotenv().ok();
    let (tx, _rx) = unbounded_channel();

    let tracer = init_trace_parser(tokio::runtime::Handle::current().clone(), tx);
    let db = Database::default();

    let classifier = Classifier::new();

    get_traces_with_meta(&tracer, &db, block_num).await
}

fn bench_tree_building(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    // massive almost gas cap block
    // https://etherscan.io/block/18672183
    let block_number = 18672183;
    let (traces, header, metadata) = rt.block_on(setup_data(block_number));
    let classifier = Classifier::new();

    c.bench_function("build 28m gas tree", |b| {
        b.iter(black_box(classifier.build_tree(traces, header, &metadata)))
    })
}

criterion_group!(tree, bench_tree_building);
criterion_main!(tree);
