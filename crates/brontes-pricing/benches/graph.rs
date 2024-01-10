use std::{collections::HashMap, env, time::Duration};

use alloy_primitives::{FixedBytes, Address};
use reth_primitives::hex;
use brontes_core::init_tracing;
use brontes_database::Pair;
use reth_db::transaction::DbTx;
use brontes_database_libmdbx::{tables::PoolCreationBlocks, AddressToProtocol, AddressToTokens, Libmdbx};
use brontes_types::exchanges::StaticBindingsDb;
use criterion::{black_box, criterion_group, criterion_main, Criterion, measurement::WallTime, BenchmarkGroup};
use reth_db::cursor::DbCursorRO;

pub const USDT_ADDRESS: Address =
    Address(FixedBytes(hex!("dac17f958d2ee523a2206206994597c13d831ec7")));

pub fn init_bench_harness() -> Libmdbx {
    init_tracing();

    let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
    Libmdbx::init_db(brontes_db_endpoint, None).unwrap()
}

pub fn load_amount_of_pools_starting_from(
    db: &Libmdbx,
    start_block: u64,
    amount: usize,
) -> (u64,HashMap<(Address, StaticBindingsDb), Pair>) {
    let tx = db.ro_tx().unwrap();
    let mut cursor = tx.cursor_read::<PoolCreationBlocks>().unwrap();
    let _ = cursor.seek(start_block);

    let binding_tx = db.ro_tx().unwrap();
    let info_tx = db.ro_tx().unwrap();
    let mut map = HashMap::default();

    let mut cur_block = 0;
     'outer: while map.len() != amount {
        let Ok(next_val) = cursor.next() else { break };
        if let Some((block, res)) = next_val {
            for pool_address in res.0 {
                let Some(protocol) = binding_tx.get::<AddressToProtocol>(pool_address).unwrap() else {
                    continue;
                };
                let Some(info) = info_tx.get::<AddressToTokens>(pool_address).unwrap() else {
                    continue;
                };
                map.insert((pool_address, protocol), Pair(info.token0, info.token1));

                if map.len() == amount {

                    cur_block = block;
                    break 'outer 
                }
            }
        }
    }

    (cur_block, map)
}

pub fn bench_graph_building(c: &mut Criterion) {
    let mut g = group(c, "pricing-graph/building");

    let db = init_bench_harness();
    let (_, fifty_thousand) = load_amount_of_pools_starting_from(&db,0, 50_000);

    g.bench_function("50_000 pool graph", move |b| {
        b.iter(|| black_box(PairGraph::init_from_hashmap(fifty_thousand)))
    };

    let (_, hundred_thousand)= load_amount_of_pools_starting_from(&db,0, 100_000);

    g.bench_function("100_000 pool graph", move |b| {
        b.iter(|| black_box(PairGraph::init_from_hashmap(hundred_thousand)))
    };

    let (_, two_hundred_thousand) = load_amount_of_pools_starting_from(&db, 0,200_000);

    g.bench_function("200_000 pool graph", move |b| {
        b.iter(|| black_box(PairGraph::init_from_hashmap(two_hundred_thousand)))
    };

    let (_, all_known_pools) = load_amount_of_pools_starting_from(&db, 0, usize::MAX);
    g.bench_function("all known pool graph", move |b| {
        b.iter(|| black_box(PairGraph::init_from_hashmap(all_known_pools)))
    };

}

pub fn bench_graph_insertions(c: &mut Criterion) {
    let mut g = group(c, "pricing-graph/insertions");
    let db= init_bench_harness();

    let (end_block, hundred_thousand) = load_amount_of_pools_starting_from(&db,0, 100_000);
    let mut graph = PairGraph::init_from_hashmap(hundred_thousand);

    g.bench_function("100_000 pool graph inserting 5000 new pools ", move |b| {
        let (_, new_entries) = load_amount_of_pools_starting_from(end_block + 1, 5000);
        b.iter(|| black_box(
                for ((address, static_binding), pair) in &new_entries {
                    graph.add_node(*pair, *address, *static_binding)
                }
        ))
    };


    let (end_block, two_hundred_thousand) = load_amount_of_pools_starting_from(&db, 0,200_000);
    let mut graph = PairGraph::init_from_hashmap(two_hundred_thousand);

    g.bench_function("200_000 pool graph inserting 5000 new pools ", move |b| {
        let (_, new_entries) = load_amount_of_pools_starting_from(end_block + 1, 5000);
        b.iter(|| black_box(
                for ((address, static_binding), pair) in &new_entries {
                    graph.add_node(*pair, *address, *static_binding)
                }
        ))
    };
}

pub fn bench_graph_path_search(c: &mut Criterion) {
    let mut g = group(c, "pricing-graph/path_search");
    let db = init_bench_harness();

    let (_, hundred_thousand)= load_amount_of_pools_starting_from(&db,0, 100_000);
    let graph = PairGraph::init_from_hashmap(hundred_thousand);
    graph.clear_pair_cache();




    g.bench_function("100_000 pool graph token pair search", move |b| {
        b.iter(|| black_box(PairGraph::init_from_hashmap(hundred_thousand)))
    };

    let (_, two_hundred_thousand) = load_amount_of_pools_starting_from(&db, 0,200_000);

    let (_, all_known_pools) = load_amount_of_pools_starting_from(&db, 0, usize::MAX);
}

criterion_group!(
    benches,
    bench_graph_building,
    bench_graph_insertions,
    bench_graph_path_search
);

criterion_main!(benches);

fn group<'a>(c: &'a mut Criterion, group_name: &str) -> BenchmarkGroup<'a, WallTime> {
    let mut g = c.benchmark_group(group_name);
    g.noise_threshold(0.03)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3))
        .sample_size(200);
    g
}
