use brontes_core::test_utils::TraceLoader;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

pub fn bench_tx_trace_parse(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let loader = rt.block_on(TraceLoader::new());

    let block = 18793182;

    c.bench_function("29,995,104 gas block", move |b| {
        b.to_async(&rt)
            .iter(|| black_box(loader.trace_block(block)))
    });
}

criterion_group!(parse, bench_tx_trace_parse);
criterion_main!(parse);
