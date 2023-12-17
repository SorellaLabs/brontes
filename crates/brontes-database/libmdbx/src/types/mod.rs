use reth_db::table::Table;
pub mod address_to_protocol;
pub mod address_to_tokens;
pub mod cex_price;
pub mod token_decimals;
mod utils;
pub(crate) use token_decimals::*;

pub trait LibmdbxData<T: Table>: Sized {
    fn into_key_val(&self) -> (T::Key, T::Value);
}
