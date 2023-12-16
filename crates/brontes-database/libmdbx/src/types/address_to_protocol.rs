use alloy_rlp::{Decodable, Encodable};
use reth_db::{
    table::{Compress, Decompress},
    DatabaseError,
};
use reth_primitives::{Address, BufMut};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sorella_db_databases::{clickhouse, Row};

use crate::{
    tables::AddressToProtocol,
    types::utils::{address_string, static_bindings},
    LibmdbxData,
};

#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Row)]
pub struct AddressToProtocolData {
    #[serde(with = "address_string")]
    pub address: Address,

    #[serde(with = "static_bindings")]
    pub classifier_name: StaticBindingsDb,
}

impl LibmdbxData<AddressToProtocol> for AddressToProtocolData {
    fn into_key_val(
        &self,
    ) -> (
        <AddressToProtocol as reth_db::table::Table>::Key,
        <AddressToProtocol as reth_db::table::Table>::Value,
    ) {
        (self.address, self.classifier_name.clone())
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug, PartialEq, Clone, Eq, Serialize, Deserialize)]
pub enum StaticBindingsDb {
    UniswapV2,
    SushiSwapV2,
    UniswapV3,
    SushiSwapV3,
    CurveCryptoSwap,
}

impl From<String> for StaticBindingsDb {
    fn from(value: String) -> Self {
        match value.as_str() {
            "UniswapV2" => StaticBindingsDb::UniswapV2,
            "SushiSwapV2" => StaticBindingsDb::SushiSwapV2,
            "UniswapV3" => StaticBindingsDb::UniswapV3,
            "SushiSwapV3" => StaticBindingsDb::SushiSwapV3,
            "CurveCryptoSwap" => StaticBindingsDb::CurveCryptoSwap,
            _ => unreachable!("no value from str: {value}"),
        }
    }
}

impl Into<String> for StaticBindingsDb {
    fn into(self) -> String {
        match self {
            StaticBindingsDb::UniswapV2 => "UniswapV2".to_string(),
            StaticBindingsDb::SushiSwapV2 => "SushiSwapV2".to_string(),
            StaticBindingsDb::UniswapV3 => "UniswapV3".to_string(),
            StaticBindingsDb::SushiSwapV3 => "SushiSwapV3".to_string(),
            StaticBindingsDb::CurveCryptoSwap => "CurveCryptoSwap".to_string(),
        }
    }
}

impl Encodable for StaticBindingsDb {
    fn encode(&self, out: &mut dyn BufMut) {
        match self {
            StaticBindingsDb::UniswapV2 => 0u64.encode(out),
            StaticBindingsDb::SushiSwapV2 => 1u64.encode(out),
            StaticBindingsDb::UniswapV3 => 2u64.encode(out),
            StaticBindingsDb::SushiSwapV3 => 3u64.encode(out),
            StaticBindingsDb::CurveCryptoSwap => 4u64.encode(out),
        }
    }
}

impl Decodable for StaticBindingsDb {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let self_int = u64::decode(buf)?;

        let this = match self_int {
            0 => StaticBindingsDb::UniswapV2,
            1 => StaticBindingsDb::SushiSwapV2,
            2 => StaticBindingsDb::UniswapV3,
            3 => StaticBindingsDb::SushiSwapV3,
            4 => StaticBindingsDb::CurveCryptoSwap,
            _ => unreachable!("no enum variant"),
        };

        Ok(this)
    }
}

impl Compress for StaticBindingsDb {
    type Compressed = Vec<u8>;

    fn compress_to_buf<B: reth_primitives::bytes::BufMut + AsMut<[u8]>>(self, buf: &mut B) {
        let mut encoded = Vec::new();
        self.encode(&mut encoded);
        buf.put_slice(&encoded);
    }
}

impl Decompress for StaticBindingsDb {
    fn decompress<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
        let binding = value.as_ref().to_vec();
        let buf = &mut binding.as_slice();
        Ok(StaticBindingsDb::decode(buf).map_err(|_| DatabaseError::Decode)?)
    }
}
