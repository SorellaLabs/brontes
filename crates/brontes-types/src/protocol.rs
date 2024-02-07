use redefined::self_convert_redefined;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};

use crate::implement_table_value_codecs_with_zc;

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
        UniswapV3,
        SushiSwapV3,
        PancakeSwapV3,
        CurveCryptoSwap,
        AaveV2,
        AaveV3,
        BalancerV1,
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

self_convert_redefined!(Protocol);
implement_table_value_codecs_with_zc!(Protocol);
