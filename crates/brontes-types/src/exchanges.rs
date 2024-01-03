use std::fmt::Display;

use alloy_rlp::{Decodable, Encodable};
use malachite::integer::conversion::string::to_string;
use reth_db::{
    table::{Compress, Decompress},
    DatabaseError,
};
use reth_primitives::BufMut;
use serde::{Deserialize, Serialize};

#[allow(non_camel_case_types)]
#[derive(Debug, PartialEq, Clone, Copy, Eq, Hash, Serialize, Deserialize)]
pub enum StaticBindingsDb {
    UniswapV2,
    SushiSwapV2,
    UniswapV3,
    SushiSwapV3,
    CurveCryptoSwap,
    AaveV2,
}

impl StaticBindingsDb {
    pub fn as_string(&self) -> String {
        match self {
            StaticBindingsDb::UniswapV2 => "UniswapV2".to_string(),
            StaticBindingsDb::SushiSwapV2 => "SushiSwapV2".to_string(),
            StaticBindingsDb::UniswapV3 => "UniswapV3".to_string(),
            StaticBindingsDb::SushiSwapV3 => "SushiSwapV3".to_string(),
            StaticBindingsDb::CurveCryptoSwap => "CurveCryptoSwap".to_string(),
            StaticBindingsDb::AaveV2 => "AaveV2".to_string(),
        }
    }
}

impl Display for StaticBindingsDb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = self.as_string();
        writeln!(f, "{string}")
    }
}

impl From<String> for StaticBindingsDb {
    fn from(value: String) -> Self {
        match value.as_str() {
            "UniswapV2" => StaticBindingsDb::UniswapV2,
            "SushiSwapV2" => StaticBindingsDb::SushiSwapV2,
            "UniswapV3" => StaticBindingsDb::UniswapV3,
            "SushiSwapV3" => StaticBindingsDb::SushiSwapV3,
            "CurveCryptoSwap" => StaticBindingsDb::CurveCryptoSwap,
            "AaveV2" => StaticBindingsDb::AaveV2,
            _ => unreachable!("no value from str: {value}"),
        }
    }
}

impl From<StaticBindingsDb> for String {
    fn from(val: StaticBindingsDb) -> Self {
        val.as_string()
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
            StaticBindingsDb::AaveV2 => 5u64.encode(out),
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
            5 => StaticBindingsDb::AaveV2,
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
        StaticBindingsDb::decode(buf).map_err(|_| DatabaseError::Decode)
    }
}
