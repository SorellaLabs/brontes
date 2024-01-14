use std::fmt::Debug;

use brontes_database_libmdbx::{types::LibmdbxData, Libmdbx};
use serde::Deserialize;

use super::{metadata::rlp::MetadataRLPInner, setup::read_parquet};
use crate::benchmarks::metadata::rlp::MetadataRLPData;

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

pub(crate) trait InitializeTable<'db, D, F>: reth_db::table::Table + Sized + 'db
where
    D: From<F>+ LibmdbxData<Self>,
    F: From<arrow::array::RecordBatch> 
{
    fn initialize_table(file: &str, libmdbx: &Libmdbx) {

        let data = read_parquet::<F>(file).into_iter().map(|d| Into::<D>::into(d)).collect();

        println!("WRITING DATA TO LIBMDBX");

        libmdbx.initialize_table(&data).unwrap();
    }
}
