include!(concat!(env!("ABI_BUILD_DIR"), "/token_to_addresses.rs"));
include!(concat!(env!("ABI_BUILD_DIR"), "/protocol_addr_set.rs"));
include!(concat!(env!("ABI_BUILD_DIR"), "/bindings.rs"));

use std::fmt::Debug;

use alloy_sol_types::{sol, SolInterface};
use once_cell::sync::Lazy;
use reth_primitives::{alloy_primitives::FixedBytes, Address, Bytes};
use reth_rpc_types::Log;

pub trait TryDecodeSol {
    type DecodingType;

    fn try_decode(call_data: &[u8]) -> Result<Self::DecodingType, alloy_sol_types::Error>;
}

/// implements the above trait for decoding on the different binding enums
#[macro_export]
macro_rules! impl_decode_sol {
    ($enum_name:ident, $inner_type:path) => {
        impl TryDecodeSol for $enum_name {
            type DecodingType = $inner_type;

            fn try_decode(call_data: &[u8]) -> Result<Self::DecodingType, alloy_sol_types::Error> {
                Self::DecodingType::abi_decode(call_data, false)
            }
        }
    };
}
