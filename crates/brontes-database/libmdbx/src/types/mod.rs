use reth_db::table::{DupSort, Table};
pub mod address_to_protocol;
pub mod address_to_tokens;
pub mod cex_price;
pub mod dex_price;
pub mod metadata;
pub mod pool_creation_block;
pub mod pool_state;
pub mod token_decimals;
pub mod traces;
pub mod utils;
pub(crate) use token_decimals::*;
pub mod mev_block;

pub trait LibmdbxData<T: Table>: Sized {
    fn into_key_val(&self) -> (T::Key, T::Value);
}

pub trait LibmdbxDupData<T: DupSort>: Sized {
    fn into_key_subkey_val(&self) -> (T::Key, T::SubKey, T::Value);
}
