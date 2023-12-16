use alloy_rlp::{Decodable, Encodable};
use brontes_database_libmdbx::types::address_to_protocol::StaticBindingsDb;
use reth_db::{
    table::{Compress, Decompress},
    DatabaseError,
};
use reth_primitives::{Address, BufMut};
use serde::{Deserialize, Serialize};

use crate::*;

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaticBindings {
    UniswapV2(UniswapV2_Enum),
    SushiSwapV2(SushiSwapV2_Enum),
    UniswapV3(UniswapV3_Enum),
    SushiSwapV3(SushiSwapV3_Enum),
    CurveCryptoSwap(CurveCryptoSwap_Enum),
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
            StaticBindings::CurveCryptoSwap(_) => Ok(StaticReturnBindings::CurveCryptoSwap(
                CurveCryptoSwap_Enum::try_decode(call_data)?,
            )),
        }
    }
}

impl From<StaticBindingsDb> for StaticBindings {
    fn from(value: StaticBindingsDb) -> Self {
        match value {
            StaticBindingsDb::UniswapV2 => StaticBindings::UniswapV2(UniswapV2_Enum::None),
            StaticBindingsDb::SushiSwapV2 => StaticBindings::SushiSwapV2(SushiSwapV2_Enum::None),
            StaticBindingsDb::UniswapV3 => StaticBindings::UniswapV3(UniswapV3_Enum::None),
            StaticBindingsDb::SushiSwapV3 => StaticBindings::SushiSwapV3(SushiSwapV3_Enum::None),
            StaticBindingsDb::CurveCryptoSwap => {
                StaticBindings::CurveCryptoSwap(CurveCryptoSwap_Enum::None)
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
    CurveCryptoSwap(CurveCryptoSwap::CurveCryptoSwapCalls),
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UniswapV2_Enum {
    None,
}
impl_decode_sol!(UniswapV2_Enum, UniswapV2::UniswapV2Calls);

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SushiSwapV2_Enum {
    None,
}
impl_decode_sol!(SushiSwapV2_Enum, SushiSwapV2::SushiSwapV2Calls);

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UniswapV3_Enum {
    None,
}
impl_decode_sol!(UniswapV3_Enum, UniswapV3::UniswapV3Calls);

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SushiSwapV3_Enum {
    None,
}
impl_decode_sol!(SushiSwapV3_Enum, SushiSwapV3::SushiSwapV3Calls);

pub trait TryDecodeSol {
    type DecodingType;

    fn try_decode(call_data: &[u8]) -> Result<Self::DecodingType, alloy_sol_types::Error>;
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CurveCryptoSwap_Enum {
    None,
}
impl_decode_sol!(CurveCryptoSwap_Enum, CurveCryptoSwap::CurveCryptoSwapCalls);
