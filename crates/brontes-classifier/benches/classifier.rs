use std::{collections::HashSet, env};

use brontes_classifier::{
    test_utils::{build_raw_test_tree, get_traces_with_meta},
    Classifier,
};
use brontes_core::{decoding::parser::TraceParser, init_tracing, test_utils::init_trace_parser};
use brontes_database::{clickhouse::Clickhouse, Metadata};
use brontes_database_libmdbx::Libmdbx;
use brontes_types::{
    normalized_actions::Actions, structured_trace::TxTrace, test_utils::force_call_action,
    tree::Node,
};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use reth_primitives::{Address, Header};
use reth_rpc_types::trace::parity::{TraceType, TransactionTrace};
use reth_tracing_ext::TracingClient;
use serial_test::serial;
use tokio::sync::mpsc::unbounded_channel;

pub async fn setup_data(block_number: u64) -> (Vec<TxTrace>, Header, Metadata) {
    init_tracing();

    dotenv::dotenv().ok();
    let (tx, _rx) = unbounded_channel();

    let tracer = init_trace_parser(tokio::runtime::Handle::current().clone(), tx);
    let db = Clickhouse::default();

    let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
    let libmdbx = Libmdbx::init_db(brontes_db_endpoint, None).unwrap();

    let classifier = Classifier::new(&libmdbx);

    get_traces_with_meta(&tracer, &db, block_number).await
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
    let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
    let libmdbx = Libmdbx::init_db(brontes_db_endpoint, None).unwrap();
    let classifier = Classifier::new(&libmdbx);

    c.bench_function("build 28m gas tree", |b| {
        b.iter(|| black_box(classifier.build_tree(traces.clone(), header.clone())))
    });
}

criterion_group!(tree, bench_tree_building);
criterion_main!(tree);
