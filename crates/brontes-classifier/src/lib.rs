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
use brontes_types::normalized_actions::Actions;
pub use impls::*;

include!(concat!(env!("ABI_BUILD_DIR"), "/token_to_addresses.rs"));
include!(concat!(env!("ABI_BUILD_DIR"), "/protocol_addr_set.rs"));
//include!(concat!(env!("ABI_BUILD_DIR"), "/bindings.rs"));

#[cfg(not(feature = "libmdbx"))]
sol!(UniswapV2, "./abis/UniswapV2.json");
#[cfg(not(feature = "libmdbx"))]
sol!(SushiSwapV2, "./abis/SushiSwapV2.json");
#[cfg(not(feature = "libmdbx"))]
sol!(UniswapV3, "./abis/UniswapV3.json");
#[cfg(not(feature = "libmdbx"))]
sol!(SushiSwapV3, "./abis/SushiSwapV3.json");

#[cfg(feature = "libmdbx")]
sol!(
    UniswapV2,
    "/Users/josephnoorchashm/Desktop/SorellaLabs/GitHub/brontes/crates/brontes-classifier/\
     abisUniswapV2.json"
);
#[cfg(feature = "libmdbx")]
sol!(
    SushiSwapV2,
    "/Users/josephnoorchashm/Desktop/SorellaLabs/GitHub/brontes/crates/brontes-classifier/\
     abisSushiSwapV2.json"
);
#[cfg(feature = "libmdbx")]
sol!(
    UniswapV3,
    "/Users/josephnoorchashm/Desktop/SorellaLabs/GitHub/brontes/crates/brontes-classifier/\
     abisUniswapV3.json"
);
#[cfg(feature = "libmdbx")]
sol!(
    SushiSwapV3,
    "/Users/josephnoorchashm/Desktop/SorellaLabs/GitHub/brontes/crates/brontes-classifier/\
     abisSushiSwapV3.json"
);

#[allow(non_camel_case_types)]
pub enum StaticBindings {
    UniswapV2(UniswapV2_Enum),
    SushiSwapV2(SushiSwapV2_Enum),
    UniswapV3(UniswapV3_Enum),
    SushiSwapV3(SushiSwapV3_Enum),
}
impl StaticBindings {
    pub fn try_decode(
        &self,
        call_data: &[u8],
    ) -> Result<StaticReturnBindings, alloy_sol_types::Error> {
        match self {
            StaticBindings::UniswapV2(_) => {
                Ok(StaticReturnBindings::UniswapV2(UniswapV2_Enum::try_decode(call_data)?))
            }
            StaticBindings::SushiSwapV2(_) => {
                Ok(StaticReturnBindings::SushiSwapV2(SushiSwapV2_Enum::try_decode(call_data)?))
            }
            StaticBindings::UniswapV3(_) => {
                Ok(StaticReturnBindings::UniswapV3(UniswapV3_Enum::try_decode(call_data)?))
            }
            StaticBindings::SushiSwapV3(_) => {
                Ok(StaticReturnBindings::SushiSwapV3(SushiSwapV3_Enum::try_decode(call_data)?))
            }
        }
    }
}

#[allow(non_camel_case_types)]
pub enum StaticReturnBindings {
    UniswapV2(UniswapV2::UniswapV2Calls),
    SushiSwapV2(SushiSwapV2::SushiSwapV2Calls),
    UniswapV3(UniswapV3::UniswapV3Calls),
    SushiSwapV3(SushiSwapV3::SushiSwapV3Calls),
}

#[allow(non_camel_case_types)]
pub enum UniswapV2_Enum {
    None,
}
impl_decode_sol!(UniswapV2_Enum, UniswapV2::UniswapV2Calls);

#[allow(non_camel_case_types)]
pub enum SushiSwapV2_Enum {
    None,
}
impl_decode_sol!(SushiSwapV2_Enum, SushiSwapV2::SushiSwapV2Calls);

#[allow(non_camel_case_types)]
pub enum UniswapV3_Enum {
    None,
}
impl_decode_sol!(UniswapV3_Enum, UniswapV3::UniswapV3Calls);

#[allow(non_camel_case_types)]
pub enum SushiSwapV3_Enum {
    None,
}
impl_decode_sol!(SushiSwapV3_Enum, SushiSwapV3::SushiSwapV3Calls);

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
