use criterion::{criterion_group, Criterion};

use super::{rlp::MetadataRLP, setup, METADATA_PARQUET_FILE, METADATA_QUERY};
use crate::benchmarks::{metadata::metadata_schema, setup::init_db, tables::InitializeTable};

fn compare_metadata(c: &mut Criterion) {
    let mut group = c.benchmark_group("Metadata");

    println!("Starting");

    setup();

    let libmdbx = init_db();

    group.bench_function("Rlp", |b| {
        b.iter(|| MetadataRLP::initialize_table(METADATA_PARQUET_FILE, &libmdbx))
    });

    group.finish();
}

criterion_group!(metadata, compare_metadata);
