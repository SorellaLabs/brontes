use std::{collections::HashMap, env, fs::metadata, path::Path, time::Duration};

use alloy_primitives::Address;
use brontes_core::{decoding::Parser, init_tracing};
use brontes_database::{
    clickhouse::USDT_ADDRESS,
    libmdbx::{
        tables::PoolCreationBlocks, types::metadata, AddressToProtocol, AddressToTokens, Libmdbx,
    },
    Pair,
};
use brontes_inspect::cex_dex::CexDexInspector;
use brontes_types::{exchanges::Protocol, normalized_actions::NormalizedAction, tree::BlockTree};
use criterion::{
    black_box, criterion_group, criterion_main, measurement::WallTime, BenchmarkGroup, Criterion,
};
use rand::seq::SliceRandom;
use reth_db::{cursor::DbCursorRO, transaction::DbTx};
use reth_tracing_ext::TracingClient;
use tokio::sync::mpsc::unbounded_channel;

pub fn init_bench_harness() -> Libmdbx {
    let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
    Libmdbx::init_db(brontes_db_endpoint, None).unwrap()
}

fn bench_cex_dex<V>(
    name: &str,
    block_tree: &mut BlockTree<V>,
    block_num: u64,
    g: &mut BenchmarkGroup<'_, WallTime>,
) where
    V: NormalizedAction,
{
    let (metrics_tx, metrics_rx) = unbounded_channel();
    let (manager, tracer) =
        TracingClient::new(Path::new(&db_path), tokio::runtime::Handle::current(), max_tasks);
    tokio::spawn(manager);

    let parser = Parser::new(
        metrics_tx,
        &libmdbx,
        tracer,
        Box::new(|address, db_tx| db_tx.get::<AddressToProtocol>(*address).unwrap().is_none()),
    );

    let parser_fut = self.parser.execute(self.block_number);
    let db = init_bench_harness();
    let cex_dex_inspectror = CexDexInspector::new(USDT_ADDRESS, &db);
    let metadata = db.get_metadata(block_num);

    // g.bench_function(name, move |b| {
    //     b.iter(|| {
    //         for ((block_num, tree, metadata), &bench_data) {
    //             black_box(cex_dex_inspector(
    //                 block_num,
    //                 &metadata,
    //                 &parser_fut,
    //                 &block_tree,
    //                 address,
    //                 static_binding,
    //                 pair,
    //             ))
    //         }
    //     })
    // });
}

criterion_group!(inspector_benches, bench_cex_dex);

criterion_main!(inspector_benches);

fn group<'a>(c: &'a mut Criterion, group_name: &str) -> BenchmarkGroup<'a, WallTime> {
    let mut g = c.benchmark_group(group_name);
    g.noise_threshold(0.03)
        .warm_up_time(Duration::from_secs(1))
        .sample_size(40);
    g
}
