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
        PartialOrd,
        Ord,
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
        BalancerV1CRP,
        UniswapX,
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
        CompoundV2,
        MakerPSM,
        #[default]
        Unknown,
    }
);

impl Protocol {
    pub fn into_clickhouse_protocol(&self) -> (&str, &str) {
        match self {
            Protocol::UniswapV2 => ("Uniswap", "V2"),
            Protocol::SushiSwapV2 => ("SushiSwap", "V2"),
            Protocol::PancakeSwapV2 => todo!(),
            Protocol::UniswapV3 => ("Uniswap", "V3"),
            Protocol::SushiSwapV3 => ("SushiSwap", "V3"),
            Protocol::PancakeSwapV3 => todo!(),
            Protocol::AaveV2 => ("Aave", "V2"),
            Protocol::AaveV3 => ("Aave", "V3"),
            Protocol::BalancerV1 => ("Balancer", "V1"),
            Protocol::BalancerV1CRP => ("Balancer", "V1SmartPool"),
            Protocol::UniswapX => ("Uniswap", "X"),
            Protocol::CurveBasePool2 => ("Curve.fi", "Base"),
            Protocol::CurveBasePool3 => ("Curve.fi", "Base"),
            Protocol::CurveBasePool4 => ("Curve.fi", "Base"),
            Protocol::CurveV1MetaPool => ("Curve.fi", "V1 Metapool"),
            Protocol::CurveV1MetapoolImpl => ("Curve.fi", "V1 Metapool Impl"),
            Protocol::CurveV2MetaPool => ("Curve.fi", "V2 Metapool"),
            Protocol::CurveV2MetapoolImpl => ("Curve.fi", "V2 Metapool Impl"),
            Protocol::CurveV2PlainPool => ("Curve.fi", "V2 Plain"),
            Protocol::CurveV2PlainPoolImpl => ("Curve.fi", "V2 Plain Impl"),
            Protocol::CurvecrvUSDMetaPool => ("Curve.fi", "crvUSD Metapool"),
            Protocol::CurvecrvUSDMetapoolImpl => ("Curve.fi", "crvUSD Metapool Impl"),
            Protocol::CurvecrvUSDPlainPool => ("Curve.fi", "crvUSD Plain"),
            Protocol::CurvecrvUSDPlainPoolImpl => ("Curve.fi", "crvUSD Plain Impl"),
            Protocol::CurveCryptoSwapPool => ("Curve.fi", "CryptoSwap"),
            Protocol::CurveTriCryptoPool => ("Curve.fi", "TriCrypto"),
            Protocol::CompoundV2 => ("Compound", "V2"),
            Protocol::MakerPSM => ("Maker", "PSM"),
            Protocol::Unknown => ("Unknown", "Unknown"),
        }
    }

    pub fn from_db_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "uniswapv2" => Protocol::UniswapV2,
            "sushiswapv2" => Protocol::SushiSwapV2,
            "uniswapv3" => Protocol::UniswapV3,
            "sushiswapv3" => Protocol::SushiSwapV3,
            "curve.fibase2" => Protocol::CurveBasePool2,
            "curve.fibase3" => Protocol::CurveBasePool3,
            "curve.fibase4" => Protocol::CurveBasePool4,
            "curve.fiv1 metapool" => Protocol::CurveV1MetaPool,
            "curve.fiv2 metapool" => Protocol::CurveV2MetaPool,
            "curve.fiv2 plain" => Protocol::CurveV2PlainPool,
            "curve.ficrvusd metapool" => Protocol::CurvecrvUSDMetaPool,
            "curve.ficrvusd plain" => Protocol::CurvecrvUSDPlainPool,
            "curve.ficryptoswap" => Protocol::CurveCryptoSwapPool,
            "curve.fitricrypto" => Protocol::CurveTriCryptoPool,
            _ => Protocol::Unknown,
        }
    }
}

self_convert_redefined!(Protocol);
implement_table_value_codecs_with_zc!(Protocol);
