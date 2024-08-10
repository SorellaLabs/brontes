#![feature(trivial_bounds)]
#![feature(associated_type_defaults)]
#![feature(const_trait_impl)]
#![feature(noop_waker)]

pub mod clickhouse;
pub mod postgres;
pub mod libmdbx;
pub mod parquet;
pub use libmdbx::{
    tables::*,
    types::{CompressedTable, IntoTableKey},
};
pub use reth_db::table::Table;
