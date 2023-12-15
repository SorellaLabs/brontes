use std::collections::HashSet;

use brontes_core::{decoding::parser::TraceParser, init_tracing, test_utils::init_trace_parser};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tokio::sync::mpsc::unbounded_channel;

fn bench_tx_trace_parse(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let block = 18793182;
    let (tx, _rx) = unbounded_channel();
    let tracer = init_trace_parser(tokio::runtime::Handle::current().clone(), tx);
    c.bench_function("29,995,104 gas block", move |b| {
        b.to_async(rt)
            .iter(|| black_box(tracer.execute_block(block)))
    });
}

criterion_group!(parsing, bench_tx_trace_parse);
criterion_main!(parsing);
