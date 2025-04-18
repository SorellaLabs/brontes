#![feature(trait_alias)]
pub mod arbitrum;
pub mod decoding;
pub mod errors;
pub mod executor;
#[cfg(not(feature = "local-reth"))]
pub mod local_provider;
pub mod missing_token_info;
pub mod rpc_client;

#[cfg(feature = "tests")]
pub mod test_utils;
#[cfg(feature = "tests")]
pub use test_utils::*;
