use redefined::self_convert_redefined;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};

use crate::implement_table_value_codecs_with_zc;

macro_rules! utils {
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
            pub fn parse_string(str: String) -> Self {
                let lower = str.to_lowercase();
                paste::paste!(
                match lower.as_str(){
                    $(
                        stringify!([<$varient:lower>]) => return Self::$varient,
                    )+
                    p => panic!("no var for {}",p)
                }
                );
            }
        }
    };
}

utils!(
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
        rSerialize,
        rDeserialize,
        Archive,
        strum::Display,
        strum::EnumString,
    )]
    #[repr(u8)]
    pub enum Protocol {
        UniswapV2,
        SushiSwapV2,
        PancakeSwapV2,
        UniswapV3,
        SushiSwapV3,
        PancakeSwapV3,
        AaveV2,
        AaveV3,
        BalancerV1,
        UniswapX,
        ZeroX,
        CurveBasePool2,
        CurveBasePool3,
        CurveBasePool4,
        CurveV1MetaPool,
        CurveV1MetapoolImpl,
        CurveV2MetaPool,
        CurveV2MetapoolImpl,
        CurveV2PlainPool,
        CurveV2PlainPoolImpl,
        CurvecrvUSDMetaPool,
        CurvecrvUSDMetapoolImpl,
        CurvecrvUSDPlainPool,
        CurvecrvUSDPlainPoolImpl,
        CurveCryptoSwapPool,
        CurveTriCryptoPool,
        MakerPSM,
        #[default]
        Unknown,
    }
);

self_convert_redefined!(Protocol);
implement_table_value_codecs_with_zc!(Protocol);
