#![allow(non_camel_case_types)]

use reth_db::table::DupSort;
pub mod address_to_factory;
pub mod address_to_protocol;
pub mod address_to_tokens;
pub mod cex_price;
pub mod dex_price;
pub mod metadata;
pub mod pool_creation_block;

pub mod subgraphs;
pub mod token_decimals;
pub mod traces;
pub mod utils;
pub(crate) use token_decimals::*;

use super::CompressedTable;

pub mod mev_block;

pub trait LibmdbxData<T: CompressedTable>: Sized
where
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
{
    fn into_key_val(&self) -> (T::Key, T::DecompressedValue);
}

pub trait LibmdbxDupData<T: DupSort + CompressedTable>: Sized
where
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
{
    fn into_key_subkey_val(&self) -> (T::Key, T::SubKey, T::DecompressedValue);
}
