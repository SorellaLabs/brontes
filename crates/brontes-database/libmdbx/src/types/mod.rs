use reth_db::table::Table;
//pub mod address_to_protocol;
pub mod address_to_tokens;
pub mod token_decimals;
mod utils;
pub(crate) use token_decimals::*;
//pub mod vec_wrapper;

pub trait LibmbdxData<T: Table>: Sized {
    fn into_key_val(&self) -> (T::Key, T::Value);
}
