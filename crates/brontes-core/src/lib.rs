#![feature(trait_alias)]
pub mod decoding;
pub mod errors;
pub mod executor;
pub mod macros;
// TODO: move into own module
pub mod dex_price;
pub mod missing_decimals;

#[cfg(feature = "tests")]
pub mod test_utils;
#[cfg(feature = "tests")]
pub use test_utils::*;
