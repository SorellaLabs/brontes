use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use brontes_database::{clickhouse::Clickhouse, libmdbx::cex_utils::CexRangeOrArbitrary};
use criterion::{black_box, criterion_group, criterion_main, Criterion, SamplingMode};
use tokio::runtime::Runtime;

fn setup_runtime_and_client() -> (Runtime, Clickhouse) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let client = rt.block_on(Clickhouse::new_default(None));

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
    let range = CexRangeOrArbitrary::Range(19000000, 19001000);
    let mut group = c.benchmark_group("get_raw_cex_quotes_range");
    group.sampling_mode(SamplingMode::Flat);
    group.sample_size(10);

    let block_times = rt
        .block_on(async { client.get_block_times_range(&range).await })
        .unwrap();

    let start = block_times.first().unwrap().timestamp;
    let end = block_times.last().unwrap().timestamp + 300 * 1_000_000;

    let quote_count = Arc::new(AtomicUsize::new(0));
    let quote_count_clone = Arc::clone(&quote_count);
    let iter_count = Arc::new(AtomicUsize::new(0));

    group.bench_function("get_raw_cex_quotes_range", |b| {
        b.to_async(&rt).iter(|| async {
            let quotes = black_box(client.get_raw_cex_quotes_range(start, end).await.unwrap());
            quote_count_clone.fetch_add(quotes.len(), Ordering::Relaxed);
            iter_count.fetch_add(1, Ordering::Relaxed);
            black_box(quotes);
        })
    });

    println!(
        "Total quote count: {}",
        quote_count.load(Ordering::Relaxed) / iter_count.load(Ordering::Relaxed)
    );
    println!(
        "Estimated total size of quotes: {} bytes",
        quote_count.load(Ordering::Relaxed) * 75 / iter_count.load(Ordering::Relaxed)
    );
    group.finish();
}

criterion_group!(
    cex_download_benches,
    bench_query_block_times,
    bench_fetch_symbol_rank,
    bench_get_raw_cex_quotes_range
);

criterion_main!(cex_download_benches);
