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

impl From<StaticBindingsDb> for StaticBindings {
    fn from(value: StaticBindingsDb) -> Self {
        match value {
            StaticBindingsDb::UniswapV2 => StaticBindings::UniswapV2(UniswapV2_Enum::None),
            StaticBindingsDb::SushiSwapV2 => StaticBindings::SushiSwapV2(SushiSwapV2_Enum::None),
            StaticBindingsDb::UniswapV3 => StaticBindings::UniswapV3(UniswapV3_Enum::None),
            StaticBindingsDb::SushiSwapV3 => StaticBindings::SushiSwapV3(SushiSwapV3_Enum::None),
        }
    }
}

impl From<String> for StaticBindings {
    fn from(value: String) -> Self {
        match value.as_str() {
            "UniswapV2" => StaticBindings::UniswapV2(UniswapV2_Enum::None),
            "SushiSwapV2" => StaticBindings::SushiSwapV2(SushiSwapV2_Enum::None),
            "UniswapV3" => StaticBindings::UniswapV3(UniswapV3_Enum::None),
            "SushiSwapV3" => StaticBindings::SushiSwapV3(SushiSwapV3_Enum::None),
            _ => unreachable!("no value from str: {value}"),
        }
    }
}

impl Into<String> for StaticBindings {
    fn into(self) -> String {
        match self {
            StaticBindings::UniswapV2(_) => "UniswapV2".to_string(),
            StaticBindings::SushiSwapV2(_) => "SushiSwapV2".to_string(),
            StaticBindings::UniswapV3(_) => "UniswapV3".to_string(),
            StaticBindings::SushiSwapV3(_) => "SushiSwapV3".to_string(),
        }
    }
}

impl Encodable for StaticBindings {
    fn encode(&self, out: &mut dyn BufMut) {
        match self {
            StaticBindings::UniswapV2(_) => 0u64.encode(out),
            StaticBindings::SushiSwapV2(_) => 1u64.encode(out),
            StaticBindings::UniswapV3(_) => 2u64.encode(out),
            StaticBindings::SushiSwapV3(_) => 3u64.encode(out),
        }
    }
}

impl Decodable for StaticBindings {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let self_int = u64::decode(buf)?;

        let this = match self_int {
            0 => StaticBindings::UniswapV2(UniswapV2_Enum::None),
            1 => StaticBindings::SushiSwapV2(SushiSwapV2_Enum::None),
            2 => StaticBindings::UniswapV3(UniswapV3_Enum::None),
            3 => StaticBindings::SushiSwapV3(SushiSwapV3_Enum::None),
            _ => unreachable!("no enum variant"),
        };

        Ok(this)
    }
}

impl Compress for StaticBindings {
    type Compressed = Vec<u8>;

    fn compress_to_buf<B: reth_primitives::bytes::BufMut + AsMut<[u8]>>(self, buf: &mut B) {
        let mut encoded = Vec::new();
        self.encode(&mut encoded);
        buf.put_slice(&encoded);
    }
}

impl Decompress for StaticBindings {
    fn decompress<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
        let binding = value.as_ref().to_vec();
        let buf = &mut binding.as_slice();
        Ok(StaticBindings::decode(buf).map_err(|_| DatabaseError::Decode)?)
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
