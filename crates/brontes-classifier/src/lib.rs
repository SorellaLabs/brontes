use std::fmt::Debug;

use once_cell::sync::Lazy;
use reth_primitives::{Address, Bytes, alloy_primitives::FixedBytes};
use reth_rpc_types::Log;

pub mod classifier;
pub use classifier::*;

#[cfg(feature = "tests")]
pub mod test_utils;

mod impls;
use alloy_sol_types::{sol, SolInterface};
use brontes_types::normalized_actions::Actions;
pub use impls::*;

include!(concat!(env!("ABI_BUILD_DIR"), "/token_to_addresses.rs"));
include!(concat!(env!("ABI_BUILD_DIR"), "/protocol_addr_set.rs"));
include!(concat!(env!("ABI_BUILD_DIR"), "/bindings.rs"));

pub trait TryDecodeSol {
    type DecodingType;

    fn try_decode(call_data: &[u8]) -> Result<Self::DecodingType, alloy_sol_types::Error>;
}

pub trait ActionCollection: Sync + Send {
    fn dispatch(
        &self,
        sig: &[u8],
        index: u64,
        data: StaticReturnBindings,
        return_data: Bytes,
        from_address: Address,
        target_address: Address,
        logs: &Vec<Log>,
    ) -> Option<Actions>;
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

pub trait IntoAction: Debug + Send + Sync {
    fn get_signature(&self) -> [u8; 4];

    fn decode_trace_data(
        &self,
        index: u64,
        data: StaticReturnBindings,
        return_data: Bytes,
        from_address: Address,
        target_address: Address,
        logs: &Vec<Log>,
    ) -> Option<Actions>;
}
