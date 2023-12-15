use std::fmt::Debug;

use once_cell::sync::Lazy;
use reth_primitives::{alloy_primitives::FixedBytes, Address, Bytes};
use reth_rpc_types::Log;

pub mod classifier;
pub use classifier::*;

#[cfg(feature = "tests")]
pub mod test_utils;

mod impls;
use alloy_sol_types::{sol, SolInterface};
pub use brontes_static;
use brontes_static::PROTOCOL_ADDRESS_MAPPING;
use brontes_types::normalized_actions::Actions;
pub use impls::*;

include!(concat!(env!("ABI_BUILD_DIR"), "/protocol_classifier_map.rs"));

pub fn fetch_classifier(
    address: Address,
) -> Option<(Lazy<Box<dyn ActionCollection>>, StaticReturnBindings)> {
    let (protocol, binding) = PROTOCOL_ADDRESS_MAPPING.get(&address.0 .0)?;
    let proto = protocol?;

    let classifier = PROTOCOL_CLASSIFIER_MAPPING.get(proto).expect("unreachable");

    Some((classifier, binding))
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
