#![feature(trait_alias)]
pub mod decoding;
pub mod errors;
pub mod executor;
#[cfg(feature = "local")]
pub mod local_provider;
pub mod missing_decimals;

#[cfg(feature = "tests")]
pub mod test_utils;
#[cfg(feature = "tests")]
pub use test_utils::*;
