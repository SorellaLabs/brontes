use std::{fmt::Debug, str::FromStr};

use brontes_database_libmdbx::types::LibmdbxData;

use crate::benchmarks::metadata::zero_copy::MetadataRkyvInner;
use crate::benchmarks::metadata::{rlp::MetadataRLPInner, MetadataBench, bincode::MetadataBincodeInner};
use crate::benchmarks::metadata::rlp::MetadataRLPData;
use reth_db::{table::Table, TableType};
use crate::libmdbx_impl::LibmdbxBench;
use crate::benchmarks::metadata::bincode::MetadataBincodeData;
use crate::benchmarks::metadata::zero_copy::MetadataRkyvData;


pub const BENCH_NUM_TABLES: usize  = 3;

pub enum BenchTables {
    MetadataRLP,
    MetadataBincode,
    MetadataRkyv
}

impl BenchTables {

    pub const ALL: [BenchTables; BENCH_NUM_TABLES] = [BenchTables::MetadataRLP, BenchTables::MetadataBincode, BenchTables::MetadataRkyv];


    pub fn table_type(&self) -> TableType {
        match self {
            BenchTables::MetadataRLP => TableType::Table,
            BenchTables::MetadataBincode => TableType::Table,
            BenchTables::MetadataRkyv => TableType::Table
        }
    }

    pub fn name(&self) -> &str {
        match self {
            BenchTables::MetadataRLP => MetadataRLP::NAME,
            BenchTables::MetadataBincode => MetadataBincode::NAME,
            BenchTables::MetadataRkyv => MetadataRkyv::NAME
        }
    }
}


impl FromStr for BenchTables {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            MetadataRLP::NAME => Ok(BenchTables::MetadataRLP),
            MetadataBincode::NAME => Ok(BenchTables::MetadataBincode),
            MetadataRkyv::NAME => Ok(BenchTables::MetadataRkyv),
            _ => Err("Unknown table".to_string()),
        }
    }
}




pub trait IntoTableKey<T, K> {
    fn into_key(value: T) -> K;
}

/// Macro to declare key value table + extra impl
#[macro_export]
macro_rules! bench_table {
    ($(#[$docs:meta])+ ( $table_name:ident ) $key:ty | $value:ty | $parquet_table_name:ty) => {
        $(#[$docs])+
        ///
        #[doc = concat!("Takes [`", stringify!($key), "`] as a key and returns [`", stringify!($value), "`].")]
        #[derive(Clone, Copy, Debug, Default)]
        pub struct $table_name;

        impl IntoTableKey<&str, $key> for $table_name {
            fn into_key(value: &str) -> $key {
                let key: $key = value.parse().unwrap();
                println!("decoded key: {key:?}");
                key
            }
        }

        impl reth_db::table::Table for $table_name {
            const NAME: &'static str = stringify!($table_name);
            type Key = $key;
            type Value = $value;
        }

        impl std::fmt::Display for $table_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", stringify!($table_name))
            }
        }

        impl<'db> InitializeTable<'db, paste::paste! {[<$table_name Data>]}, $parquet_table_name> for $table_name {}
    };
}

pub(crate) trait InitializeTable<'db, D, F>: reth_db::table::Table + Sized + Default + 'db
where
    D: From<F>+ LibmdbxData<Self>,
    F: From<arrow::array::RecordBatch> + Clone
{
    fn initialize_table(libmdbx: &LibmdbxBench, data: &Vec<F>) {

        //println!("WRITING DATA TO LIBMDBX");

        let libmdbx_data = data
        .iter()
        .map(|d| Into::<D>::into(d.clone()))
        .collect::<Vec<_>>();

        libmdbx.initialize_table(&libmdbx_data).unwrap();
    }
}


bench_table!(
    /// rlp metadata
    ( MetadataRLP ) u64 | MetadataRLPInner | MetadataBench
);

bench_table!(
    /// bincode metadata
    ( MetadataBincode ) u64 | MetadataBincodeInner | MetadataBench
);


bench_table!(
    /// rkyv metadata
    ( MetadataRkyv ) u64 | MetadataRkyvInner | MetadataBench
);

