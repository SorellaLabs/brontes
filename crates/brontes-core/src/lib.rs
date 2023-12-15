#![feature(trait_alias)]
pub mod decoding;
pub mod dex_price;
pub mod errors;
pub mod executor;
pub mod macros;
pub mod missing_decimals;

#[cfg(feature = "tests")]
pub mod test_utils;
#[cfg(feature = "tests")]
pub use test_utils::*;

// include!(concat!(env!("ABI_BUILD_DIR"), "/dex_price_map.rs"));
