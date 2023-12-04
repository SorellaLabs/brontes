#![feature(trait_alias)]
pub mod decoding;
pub mod errors;
pub mod executor;
pub mod macros;
// TODO: move into own module
pub mod missing_decimals;
pub mod dex_price;

#[cfg(feature = "tests")]
pub mod test_utils;
#[cfg(feature = "tests")]
pub use test_utils::*;
