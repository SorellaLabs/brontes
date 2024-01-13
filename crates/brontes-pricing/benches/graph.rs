use std::{collections::HashMap, env, time::Duration};

use alloy_primitives::Address;
use brontes_core::init_tracing;
use brontes_database::{clickhouse::USDT_ADDRESS, Pair};
use brontes_database_libmdbx::{
    tables::PoolCreationBlocks, AddressToProtocol, AddressToTokens, Libmdbx,
};
use brontes_pricing::AllPairGraph as PairGraph;
use brontes_types::exchanges::StaticBindingsDb;
use criterion::{
    black_box, criterion_group, criterion_main, measurement::WallTime, BenchmarkGroup, Criterion,
};
use rand::seq::SliceRandom;
use reth_db::{cursor::DbCursorRO, transaction::DbTx};

pub fn init_bench_harness() -> Libmdbx {
    let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
    Libmdbx::init_db(brontes_db_endpoint, None).unwrap()
}

pub fn load_amount_of_pools_starting_from(
    db: &Libmdbx,
    start_block: u64,
    amount: usize,
) -> (u64, HashMap<(Address, StaticBindingsDb), Pair>) {
    let tx = db.ro_tx().unwrap();
    let mut cursor = tx.cursor_read::<PoolCreationBlocks>().unwrap();
    let _ = cursor.seek(start_block);

    let binding_tx = db.ro_tx().unwrap();
    let info_tx = db.ro_tx().unwrap();
    let mut map = HashMap::default();

    let mut cur_block = 0;
    'outer: while map.len() != amount {
        let Ok(next_val) = cursor.next() else { break 'outer };
        let Some((block, res)) = next_val else { break 'outer };

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

    (cur_block, map)
}

pub fn bench_graph_building(c: &mut Criterion) {
    let mut g = group(c, "pricing-graph/building");

    let db = init_bench_harness();
    let (_, fifty_thousand) = load_amount_of_pools_starting_from(&db, 0, 50_000);

    g.bench_function("50_000 pool graph", move |b| {
        b.iter(|| black_box(PairGraph::init_from_hashmap(fifty_thousand.clone())))
    });

    let (_, hundred_thousand) = load_amount_of_pools_starting_from(&db, 0, 100_000);

    g.bench_function("100_000 pool graph", move |b| {
        b.iter(|| black_box(PairGraph::init_from_hashmap(hundred_thousand.clone())))
    });

    let (_, two_hundred_thousand) = load_amount_of_pools_starting_from(&db, 0, 200_000);

    g.bench_function("200_000 pool graph", move |b| {
        b.iter(|| black_box(PairGraph::init_from_hashmap(two_hundred_thousand.clone())))
    });

    let (_, all_known_pools) = load_amount_of_pools_starting_from(&db, 0, usize::MAX);
    g.bench_function("all known pool graph", move |b| {
        b.iter(|| black_box(PairGraph::init_from_hashmap(all_known_pools.clone())))
    });
}

pub fn bench_graph_insertions(c: &mut Criterion) {
    let mut g = group(c, "pricing-graph/insertions");
    let db = init_bench_harness();

    let (end_block, hundred_thousand) = load_amount_of_pools_starting_from(&db, 0, 100_000);
    let graph = PairGraph::init_from_hashmap(hundred_thousand);
    let (_, new_entries) = load_amount_of_pools_starting_from(&db, end_block + 1, 5000);
    bench_insertions("100_000 pool graph inserting 5000 new pools", graph, &mut g, new_entries);

    let (end_block, two_hundred_thousand) = load_amount_of_pools_starting_from(&db, 0, 200_000);
    let graph = PairGraph::init_from_hashmap(two_hundred_thousand);
    let (_, new_entries) = load_amount_of_pools_starting_from(&db, end_block + 1, 5000);
    bench_insertions("200_000 pool graph inserting 5000 new pools", graph, &mut g, new_entries);
}

fn bench_insertions(
    name: &str,
    mut graph: PairGraph,
    g: &mut BenchmarkGroup<'_, WallTime>,
    new_entries: HashMap<(Address, StaticBindingsDb), Pair>,
) {
    g.bench_function(name, move |b| {
        b.iter(|| {
            for ((address, static_binding), pair) in &new_entries {
                black_box(graph.add_node(*pair, *address, *static_binding))
            }
        })
    });
}

pub fn bench_yen_graph_path_search(c: &mut Criterion) {
    let mut g = group(c, "pricing-graph/yen_path_search");
    let db = init_bench_harness();

    let (_, fifty_thousand) = load_amount_of_pools_starting_from(&db, 0, 100_000);
    println!("loaded from db");
    bench_yen_path_search(
        "yen path search graph 100_000 pools, 10 pairs to usdt",
        PairGraph::init_from_hashmap(fifty_thousand),
        &mut g,
    );

    let (_, two_hundred_thousand) = load_amount_of_pools_starting_from(&db, 0, 200_000);
    bench_yen_path_search(
        "yen path search graph 200_000 pools, 10 pairs to usdt",
        PairGraph::init_from_hashmap(two_hundred_thousand),
        &mut g,
    );

    let (_, all_known_pools) = load_amount_of_pools_starting_from(&db, 0, usize::MAX);
    bench_yen_path_search(
        "yen path search graph all pools, 10 pairs to usdt",
        PairGraph::init_from_hashmap(all_known_pools),
        &mut g,
    );
}

fn bench_yen_path_search(name: &str, mut graph: PairGraph, g: &mut BenchmarkGroup<'_, WallTime>) {
    graph.clear_pair_cache();
    let copy_graph = graph.clone();

    g.bench_function(name, move |b| {
        b.iter_batched(
            || {
                copy_graph
                    .get_all_known_addresses()
                    .choose_multiple(&mut rand::thread_rng(), 10)
                    .map(|address| Pair(*address, USDT_ADDRESS))
                    .collect::<Vec<Pair>>()
            },
            |test_pairs| {
                for pair in test_pairs {
                    black_box(graph.get_k_paths_no_cache(pair));
                }
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

criterion_group!(graph_building_benches, bench_graph_building);
criterion_group!(graph_insertions_benches, bench_graph_insertions);
criterion_group!(yen_graph_path_search_benches, bench_yen_graph_path_search);

criterion_main!(
    graph_building_benches,
    graph_insertions_benches,
    graph_path_search_benches,
    yen_graph_path_search_benches
);

fn group<'a>(c: &'a mut Criterion, group_name: &str) -> BenchmarkGroup<'a, WallTime> {
    let mut g = c.benchmark_group(group_name);
    g.noise_threshold(0.03)
        .warm_up_time(Duration::from_secs(1))
        .sample_size(40);
    g
}
