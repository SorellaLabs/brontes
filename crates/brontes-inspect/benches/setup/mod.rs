use std::{collections::HashMap, env, fs::metadata, path::Path, time::Duration};

use alloy_primitives::Address;
use brontes_core::{decoding::Parser, init_tracing};
use brontes_database::{clickhouse::USDT_ADDRESS, Pair};
use brontes_database_libmdbx::{
    tables::PoolCreationBlocks, types::metadata, AddressToProtocol, AddressToTokens, Libmdbx,
};
use brontes_inspect::cex_dex::CexDexInspector;
use brontes_types::{
    exchanges::StaticBindingsDb, normalized_actions::NormalizedAction, tree::BlockTree,
};
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

pub struct CexDexBenchData {
    pub block_num: u64,
    pub metadata:  Metadata,
    pub tree:      BlockTree<NormalizedAction>,
    pub inspector: CexDexInspector,
}

fn get_cex_dex_bench_data() -> CexDexBenchData {
    block = vec![16802196, 16815617, 18264694, 18971364];

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

    let parser_fut = parser.execute(block_num);
    let db = init_bench_harness();
    let cex_dex_inspectror = CexDexInspector::new(USDT_ADDRESS, &db);
    let metadata = db.get_metadata(block_num);
}
