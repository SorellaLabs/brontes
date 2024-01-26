#![allow(non_camel_case_types)]
pub mod address_to_protocol;
pub mod address_to_tokens;
pub mod cex_price;
pub mod dex_price;
pub mod metadata;
pub mod mev_block;
pub mod pool_creation_block;
pub mod subgraphs;
pub mod token_decimals;
pub mod traces;
pub mod utils;

use std::fmt::Debug;

use reth_db::table::{DupSort, Table};

pub struct ReturnKV<T>
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
{
    pub key:   T::Key,
    pub value: T::DecompressedValue,
}

impl<T> From<(T::Key, T::DecompressedValue)> for ReturnKV<T>
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
{
    fn from(value: (T::Key, T::DecompressedValue)) -> Self {
        Self { key: value.0, value: value.1 }
    }
}

pub trait LibmdbxData<T: CompressedTable>: Sized + Send + Sync
where
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
{
    fn into_key_val(&self) -> ReturnKV<T>;
}

pub trait LibmdbxDupData<T: DupSort + CompressedTable>: Sized
where
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
{
    fn into_key_subkey_val(&self) -> (T::Key, T::SubKey, T::DecompressedValue);
}

pub trait IntoTableKey<T, K, D> {
    fn into_key(value: T) -> K;
    fn into_table_data(key: T, value: T) -> D;
}

pub trait CompressedTable: reth_db::table::Table + Send + Sync
where
    <Self as Table>::Value: From<<Self as CompressedTable>::DecompressedValue>
        + Into<<Self as CompressedTable>::DecompressedValue>,
{
    type DecompressedValue: Debug;
    const INIT_CHUNK_SIZE: Option<usize>;
    const INIT_QUERY: Option<&'static str>;
}
