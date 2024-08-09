use std::time::Instant;

use brontes_database::{clickhouse::Clickhouse, libmdbx::cex_utils::CexRangeOrArbitrary};
use brontes_types::{
    db::{
        block_times::BlockTimes,
        cex::{
            quotes::{
                approximate_size_of_converter, correct_usdc_address, size_of_cex_price_map,
                CexQuotesConverter, RawCexQuotes,
            },
            BestCexPerPair, CexExchange, CexSymbols,
        },
    },
    pair::Pair,
    FastHashMap,
};
use criterion::{black_box, criterion_group, criterion_main, Criterion, SamplingMode};
use itertools::Itertools;
use tokio::runtime::Runtime;

async fn fetch_test_data(
    client: &Clickhouse,
    range: CexRangeOrArbitrary,
) -> eyre::Result<(Vec<BlockTimes>, Vec<CexSymbols>, Vec<RawCexQuotes>, Vec<BestCexPerPair>)> {
    let block_times = client.get_block_times_range(&range).await?;
    let symbols = client.get_cex_symbols().await?;
    let raw_quotes = client.get_raw_cex_quotes_range(&range).await?;
    let symbol_rank = client.fetch_symbol_rank(&block_times, &range).await?;

    Ok((block_times, symbols, raw_quotes, symbol_rank))
}

fn setup_runtime_and_client() -> (Runtime, Clickhouse) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let client = rt.block_on(Clickhouse::new_default());

    (rt, client)
}

fn bench_query_block_times(c: &mut Criterion) {
    let (rt, client) = setup_runtime_and_client();
    let range = CexRangeOrArbitrary::Range(19000000, 19030000);

    let mut group = c.benchmark_group("query_block_times");
    group.sampling_mode(SamplingMode::Linear);
    group.sample_size(10);
    group.bench_function("query_block_times", |b| {
        b.to_async(&rt)
            .iter(|| async { black_box(client.get_block_times_range(&range).await.unwrap()) });
    });
    group.finish();
}

fn bench_fetch_symbol_rank(c: &mut Criterion) {
    let (rt, client) = setup_runtime_and_client();
    let range = CexRangeOrArbitrary::Range(19000000, 19030000);
    let block_times = rt.block_on(client.get_block_times_range(&range)).unwrap();

    let mut group = c.benchmark_group("fetch_symbol_rank");
    group.sampling_mode(SamplingMode::Linear);
    group.sample_size(10);
    group.bench_function("fetch_symbol_rank", |b| {
        b.to_async(&rt).iter(|| async {
            black_box(
                client
                    .fetch_symbol_rank(&block_times, &range)
                    .await
                    .unwrap(),
            )
        });
    });
    group.finish();
}

fn bench_get_raw_cex_quotes_range(c: &mut Criterion) {
    let (rt, client) = setup_runtime_and_client();
    let range = CexRangeOrArbitrary::Range(19000000, 19030000);

    let mut group = c.benchmark_group("get_raw_cex_quotes_range");
    group.sampling_mode(SamplingMode::Linear);
    group.sample_size(10);
    group.bench_function("get_raw_cex_quotes_range", |b| {
        b.to_async(&rt).iter(|| async {
            black_box(client.get_raw_cex_quotes_range(&range).await.unwrap());
        });
    });
    group.finish();
}

fn bench_full_conversion_process(c: &mut Criterion) {
    let (rt, client) = setup_runtime_and_client();
    let range = CexRangeOrArbitrary::Range(19000000, 19030000);
    let (block_times, symbols, quotes, best_cex_per_pair) =
        rt.block_on(async { fetch_test_data(&client, range).await.unwrap() });

    let mut group = c.benchmark_group("Full Conversion Process");
    group.sampling_mode(SamplingMode::Linear);
    group.sample_size(10);

    // Benchmark the conversion process
    group.bench_function("full_conversion_process", |b| {
        b.iter_custom(|iters| {
            let start = Instant::now();
            for _ in 0..iters {
                let converter = CexQuotesConverter::new(
                    block_times.clone(),
                    symbols.clone(),
                    quotes.clone(),
                    best_cex_per_pair.clone(),
                );
                black_box(converter.convert_to_prices());
            }
            start.elapsed()
        });
    });

    // Collect additional measurements separately
    let converter = CexQuotesConverter::new(block_times, symbols, quotes, best_cex_per_pair);
    let converter_size = approximate_size_of_converter(&converter);
    let quote_count = converter.quotes.len();
    let result = converter.convert_to_prices();

    let total_result_size: usize = result
        .iter()
        .map(|(_, price_map)| size_of_cex_price_map(price_map))
        .sum();
    let avg_price_map_size = total_result_size / result.len();
    let num_blocks = result.len();

    println!("Average price map size: {} bytes", avg_price_map_size);
    println!("Converter size for {} blocks: {} bytes", num_blocks, converter_size);
    println!("Quote count: {}", quote_count);

    group.finish();
}

fn bench_conversion_parts(c: &mut Criterion) {
    let (rt, client) = setup_runtime_and_client();
    let range = CexRangeOrArbitrary::Range(19000000, 19030000);
    let (block_times, symbols, quotes, best_cex_per_pair) =
        rt.block_on(async { fetch_test_data(&client, range).await.unwrap() });

    let mut converter = CexQuotesConverter::new(block_times, symbols, quotes, best_cex_per_pair);

    let mut group = c.benchmark_group("Conversion Parts");
    group.sampling_mode(SamplingMode::Linear);
    group.sample_size(10);

    group.bench_function("create_block_num_map_with_pairs", |b| {
        b.iter(|| black_box(converter.create_block_num_map_with_pairs()));
    });

    let block_num_map = converter.create_block_num_map_with_pairs();

    group.bench_function("process_best_cex_venues", |b| {
        b.iter(|| {
            black_box(
                converter.process_best_cex_venues(block_num_map.values().next().unwrap().1.clone()),
            )
        });
    });

    group.bench_function("create_price_map", |b| {
        b.iter(|| {
            let (block_num, block_time) = *block_num_map.keys().next().unwrap();

            black_box(
                converter.create_price_map(
                    block_num_map[&(block_num, block_time)].0.clone(),
                    block_time,
                ),
            )
        });
    });

    group.finish();
}

fn bench_find_closest_to_time_boundary(c: &mut Criterion) {
    let (rt, client) = setup_runtime_and_client();
    let range = CexRangeOrArbitrary::Range(19000000, 19030000);
    let (block_times, symbols, quotes, best_cex_per_pair) =
        rt.block_on(async { fetch_test_data(&client, range).await.unwrap() });

    let mut converter = CexQuotesConverter::new(block_times, symbols, quotes, best_cex_per_pair);
    let block_num_map = converter.create_block_num_map_with_pairs();

    let test_data = prepare_test_data(&mut converter, block_num_map);

    let mut group = c.benchmark_group("find_closest_to_time_boundary");
    group.sampling_mode(SamplingMode::Linear);
    group.sample_size(100);

    group.bench_function("find_closest_to_time_boundary", |b| {
        let mut index = 0;
        let data_len = test_data.len();
        b.iter(|| {
            let (block_time, exchange_pair_index_map) = &test_data[index % data_len];
            index += 1;

            black_box(
                converter
                    .find_closest_to_time_boundary(*block_time, exchange_pair_index_map.clone()),
            );
        });
    });
    group.finish();

    // Print some statistics about the test data
    println!("Number of different block times tested: {}", test_data.len());
    println!(
        "Block time range: {} to {}",
        test_data.iter().map(|(bt, _)| bt).min().unwrap_or(&0),
        test_data.iter().map(|(bt, _)| bt).max().unwrap_or(&0)
    );
}

fn prepare_test_data(
    converter: &mut CexQuotesConverter,
    data: FastHashMap<(u64, u64), (FastHashMap<CexExchange, Vec<usize>>, Vec<BestCexPerPair>)>,
) -> Vec<(u64, FastHashMap<Pair, Vec<usize>>)> {
    data.into_iter()
        .map(|((_, block_time), (exchange_maps, _))| {
            let mut exchange_pair_index_map: std::collections::HashMap<
                Pair,
                Vec<usize>,
                ahash::RandomState,
            > = FastHashMap::default();

            exchange_maps.into_iter().for_each(|(_, index)| {
                index.into_iter().for_each(|index| {
                    let quote = &converter.quotes[index];

                    let symbol = converter
                        .symbols
                        .get_mut(&(quote.exchange, quote.symbol.clone()))
                        .unwrap();

                    correct_usdc_address(&mut symbol.address_pair);

                    exchange_pair_index_map
                        .entry(symbol.address_pair)
                        .or_default()
                        .push(index);
                });
            });

            (block_time, exchange_pair_index_map)
        })
        .collect_vec()
}

criterion_group!(
    cex_download_benches,
    bench_query_block_times,
    bench_fetch_symbol_rank,
    bench_get_raw_cex_quotes_range
);

criterion_group!(
    cex_conversion_benches,
    bench_full_conversion_process,
    bench_conversion_parts,
    bench_find_closest_to_time_boundary,
);

criterion_main!(cex_download_benches, cex_conversion_benches);
