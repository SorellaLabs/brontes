use std::{collections::HashSet, env, fs::File, io::Write, path::Path};

use brontes_core::{decoding::parser::TraceParser, init_tracing, test_utils::init_trace_parser};
use brontes_database_libmdbx::Libmdbx;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tokio::sync::mpsc::unbounded_channel;

pub fn bench_tx_trace_parse(c: &mut Criterion) {
    init_tracing();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
    let libmdbx = Libmdbx::init_db(brontes_db_endpoint, None).unwrap();

    let block = 18793182;
    let (tx, _rx) = unbounded_channel();
    let tracer = rt.block_on(async move {
        init_trace_parser(
            tokio::runtime::Handle::current().clone(),
            tx,
            Box::leak(Box::new(libmdbx)),
            69,
        )
    });
    let a = rt
        .block_on(async { tracer.execute_block(block).await })
        .unwrap()
        .0;

    let dest_path = Path::new("./yeet.json");
    let mut f = File::create(dest_path).unwrap();
    writeln!(f, "{}", serde_json::to_string(&a).unwrap()).unwrap();

    println!("running bench");
    c.bench_function("29,995,104 gas block", move |b| {
        b.to_async(&rt)
            .iter(|| black_box(tracer.execute_block(block)))
    });
}

criterion_group!(parse, bench_tx_trace_parse);
criterion_main!(parse);
