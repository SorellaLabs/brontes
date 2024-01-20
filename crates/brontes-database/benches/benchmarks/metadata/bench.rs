use std::time::Duration;

use criterion::{criterion_group, measurement::WallTime, BenchmarkGroup, Criterion};
use human_bytes::human_bytes;

use self::{bincode::MetadataBincodeData, zero_copy::MetadataRkyvData};
use super::{rlp::MetadataRLPData, METADATA_QUERY};
use crate::{
    benchmarks::metadata::*,
    setup::{tables::*, utils::*},
};

fn setup(setup_parquet: bool, setup_libmdbx: bool) -> LibmdbxBench {
    dotenv::dotenv().ok();

    println!("Initializing Database");
    let tables =
        [BenchTables::MetadataRLP, BenchTables::MetadataBincode, BenchTables::MetadataRkyv];
    let libmdbx = init_db(&metadata_libmdbx_dir(), &tables);

    if setup_parquet {
        println!("Initializing Parquet File");
        parquet_setup::<MetadataBench>(METADATA_QUERY, &metadata_paquet_file(), metadata_schema());
    }

    if setup_libmdbx {
        println!("Reading Parquet File");
        let data = read_parquet::<MetadataBench>(&metadata_paquet_file());

        println!("Initializing Metadata RLP Table");
        MetadataRLP::initialize_table(&libmdbx, &data);

        println!("Initializing Metadata Bincode Table");
        MetadataBincode::initialize_table(&libmdbx, &data);

        println!("Initializing Metadata RKYV Table");
        MetadataRkyv::initialize_table(&libmdbx, &data);
    }

    libmdbx
}

fn compare_metadata(c: &mut Criterion) {
    let mut group = c.benchmark_group("Metadata");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(200));

    println!("Starting Metadata Bench");

    let setup_parquet = false;
    let setup_libmdbx = false;
    let libmdbx = setup(setup_parquet, setup_libmdbx);

    //compare_metadata_read(&mut group, &libmdbx);
    // compare_metadata_write(&mut group, &libmdbx);
    compare_metadata_size(&libmdbx);

    group.finish();
}

fn compare_metadata_read(group: &mut BenchmarkGroup<'_, WallTime>, libmdbx: &LibmdbxBench) {
    group.bench_function("Rlp Read", |b| {
        b.iter(|| libmdbx.bench_read_full_table::<MetadataRLP>("Rlp"))
    });
    group.bench_function("Bincode Read", |b| {
        b.iter(|| libmdbx.bench_read_full_table::<MetadataBincode>("Bincode"))
    });
    group.bench_function("Rkyv Read", |b| {
        b.iter(|| libmdbx.bench_read_full_table::<MetadataRkyv>("Rkyv"))
    });
}

fn compare_metadata_write(group: &mut BenchmarkGroup<'_, WallTime>, libmdbx: &LibmdbxBench) {
    let data = read_parquet::<MetadataBench>(&metadata_paquet_file());

    let rlp_data = data.iter().map(|d| d.clone().into()).collect::<Vec<_>>();
    group.bench_function("Rlp Write", |b| {
        b.iter(|| libmdbx.bench_write_full_table::<MetadataRLP, MetadataRLPData>(&rlp_data))
    });

    let bincode_data = data.iter().map(|d| d.clone().into()).collect::<Vec<_>>();
    group.bench_function("Bincode Write", |b| {
        b.iter(|| {
            libmdbx.bench_write_full_table::<MetadataBincode, MetadataBincodeData>(&bincode_data)
        })
    });

    let rkyv_data = data.iter().map(|d| d.clone().into()).collect::<Vec<_>>();
    group.bench_function("Rkyv Write", |b| {
        b.iter(|| libmdbx.bench_write_full_table::<MetadataRkyv, MetadataRkyvData>(&rkyv_data))
    });
}

fn compare_metadata_size(libmdbx: &LibmdbxBench) {
    let rlp_size = libmdbx.size_of_table::<MetadataRLP>();
    println!("\nRLP COMPRESSED SIZE: {} B -- {}", rlp_size, human_bytes(rlp_size as f64));

    let bincode_size = libmdbx.size_of_table::<MetadataBincode>();
    println!("BINCODE COMPRESSED SIZE: {} B -- {}", bincode_size, human_bytes(bincode_size as f64));

    let rkyv_size = libmdbx.size_of_table::<MetadataRkyv>();
    println!("RKYV COMPRESSED SIZE: {} B -- {}", rkyv_size, human_bytes(rkyv_size as f64));
}

criterion_group!(metadata, compare_metadata);
