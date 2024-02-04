use alloy_rlp::{Decodable, Encodable};
use redefined::{self_convert_redefined, RedefinedConvert};
use reth_db::{
    table::{Compress, Decompress},
    DatabaseError,
};
use reth_primitives::BufMut;
use rkyv::Deserialize as rkyv_Deserialize;
use serde::{Deserialize, Serialize};

macro_rules! to_byte {
    ($(#[$attr:meta])* pub enum $name:ident { $(
                $(#[$fattr:meta])*
                $varient:ident,
                )*
            }
    ) => {
        $(#[$attr])*
        pub enum $name {
            $(
                $(#[$fattr])*
                $varient
            ),+
        }

        impl $name {
            pub const fn to_byte(&self) -> u8 {
                match self {
                    $(
                        Self::$varient => Self::$varient as u8,
                    ) +
                }
            }
        }

    };
}

to_byte!(
    #[allow(non_camel_case_types)]
    #[derive(
        Debug,
        Default,
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
    #[archive(check_bytes)]
    #[repr(u8)]
    pub enum Protocol {
        UniswapV2,
        SushiSwapV2,
        UniswapV3,
        SushiSwapV3,
        PancakeSwapV3,
        CurveCryptoSwap,
        AaveV2,
        AaveV3,
        UniswapX,
        CurveV1BasePool,
        CurveV1MetaPool,
        CurveV2BasePool,
        CurveV2MetaPool,
        CurveV2PlainPool,
        MakerPSM,
        #[default]
        Unknown,
    }
);

impl Encodable for Protocol {
    fn encode(&self, out: &mut dyn BufMut) {
        let encoded = rkyv::to_bytes::<_, 256>(self).unwrap();
        out.put_slice(&encoded)
    }
}

impl Decodable for Protocol {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let archived: &ArchivedProtocol = rkyv::check_archived_root::<Self>(buf).unwrap();

        let this = ArchivedProtocol::deserialize(&archived, &mut rkyv::Infallible).unwrap();
        Ok(this)
    }
}

impl Compress for Protocol {
    type Compressed = Vec<u8>;

    fn compress_to_buf<B: reth_primitives::bytes::BufMut + AsMut<[u8]>>(self, buf: &mut B) {
        let mut encoded = Vec::new();
        self.encode(&mut encoded);
        buf.put_slice(&encoded);
    }
}

impl Decompress for Protocol {
    fn decompress<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
        let binding = value.as_ref().to_vec();
        let buf = &mut binding.as_slice();
        Protocol::decode(buf).map_err(|_| DatabaseError::Decode)
    }
}

self_convert_redefined!(Protocol);
