use alloy_rlp::{Decodable, Encodable};
use redefined::{self_convert_redefined, RedefinedConvert};
use reth_db::{
    table::{Compress, Decompress},
    DatabaseError,
};
use reth_primitives::BufMut;
use serde::{Deserialize, Serialize};

#[allow(non_camel_case_types)]
#[derive(
    Debug,
    PartialEq,
    Clone,
    Copy,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    strum::Display,
    strum::EnumString,
)]
pub enum StaticBindingsDb {
    UniswapV2,
    SushiSwapV2,
    UniswapV3,
    SushiSwapV3,
    CurveCryptoSwap,
    AaveV2,
    AaveV3,
    UniswapX,
    CurveV1BasePool,
    CurveV1MetaPool,
    CurveV2BasePool,
    CurveV2MetaPool,
    CurveV2PlainPool,
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
            StaticBindingsDb::AaveV3 => 6u64.encode(out),
            StaticBindingsDb::UniswapX => 7u64.encode(out),
            StaticBindingsDb::CurveV1BasePool => 8u64.encode(out),
            StaticBindingsDb::CurveV1MetaPool => 9u64.encode(out),
            StaticBindingsDb::CurveV2BasePool => 10u64.encode(out),
            StaticBindingsDb::CurveV2MetaPool => 11u64.encode(out),
            StaticBindingsDb::CurveV2PlainPool => 12u64.encode(out),
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
            6 => StaticBindingsDb::AaveV3,
            7 => StaticBindingsDb::UniswapX,
            8 => StaticBindingsDb::CurveV1BasePool,
            9 => StaticBindingsDb::CurveV1MetaPool,
            10 => StaticBindingsDb::CurveV2BasePool,
            11 => StaticBindingsDb::CurveV2MetaPool,
            12 => StaticBindingsDb::CurveV2PlainPool,
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

self_convert_redefined!(StaticBindingsDb);
