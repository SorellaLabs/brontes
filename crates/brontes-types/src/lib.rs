use std::{collections::HashMap, sync::OnceLock};

use malachite::{
    num::{arithmetic::traits::Pow, conversion::traits::RoundingFrom},
    rounding_modes::RoundingMode,
    Natural, Rational,
};
use parking_lot::RwLock;
use reth_primitives::U256;

pub mod classified_mev;
pub mod extra_processing;
pub mod normalized_actions;
pub mod structured_trace;
pub mod traits;
pub mod tree;

#[cfg(feature = "tests")]
pub mod test_utils;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Dexes {
    UniswapV2,
    UniswapV3,
    SushiSwapV2,
    SushiSwapV3,
    CurveCryptoSwap,
}
// include!(concat!(env!("ABI_BUILD_DIR"), "/token_mapping.rs"));

static DYN_MAP: OnceLock<RwLock<HashMap<[u8; 20], u8>>> = OnceLock::new();

pub fn try_get_decimals(address: &[u8; 20]) -> Option<u8> {
    // if let Some(value) = TOKEN_TO_DECIMALS.get(address) {
    //     Some(*value)
    // } else {
    //     DYN_MAP
    //         .get_or_init(|| RwLock::new(HashMap::new()))
    //         .read()
    //         .get(address)
    //         .copied()
    // }
    None
}

pub fn cache_decimals(address: [u8; 20], decimals: u8) {
    DYN_MAP
        .get_or_init(|| RwLock::new(HashMap::new()))
        .write()
        .insert(address, decimals);
}

pub trait ToScaledRational {
    fn to_scaled_rational(self, decimals: u8) -> Rational;
}

impl ToScaledRational for U256 {
    fn to_scaled_rational(self, decimals: u8) -> Rational {
        let top = Natural::from_limbs_asc(&self.into_limbs());

        Rational::from_naturals(top, Natural::from(10u8).pow(decimals as u64))
    }
}

impl ToScaledRational for u64 {
    fn to_scaled_rational(self, decimals: u8) -> Rational {
        let top = Natural::from(self);

        Rational::from_naturals(top, Natural::from(10u8).pow(decimals as u64))
    }
}

impl ToScaledRational for u128 {
    fn to_scaled_rational(self, decimals: u8) -> Rational {
        let top = Natural::from(self);
        Rational::from_naturals(top, Natural::from(10u8).pow(decimals as u64))
    }
}

impl ToScaledRational for i128 {
    fn to_scaled_rational(self, decimals: u8) -> Rational {
        let top = Rational::from(self);
        let bottom = Rational::from(10u8).pow(decimals as u64);
        top / bottom
    }
}

pub trait ToFloatNearest {
    fn to_float(self) -> f64;
}

impl ToFloatNearest for Rational {
    fn to_float(self) -> f64 {
        f64::rounding_from(self, RoundingMode::Nearest).0
    }
}

pub mod vec_u256 {
    use alloy_primitives::U256;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    pub fn serialize<S: Serializer>(u: &Vec<U256>, serializer: S) -> Result<S::Ok, S::Error> {
        u.iter()
            .map(|u| u.to_le_bytes())
            .collect::<Vec<[u8; 32]>>()
            .serialize(serializer)
    }

    #[allow(dead_code)]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<U256>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let u: Vec<[u8; 32]> = Deserialize::deserialize(deserializer)?;
        Ok(u.into_iter().map(U256::from_le_bytes).collect())
    }
}
pub(crate) mod vec_vec_u256 {

    use alloy_primitives::U256;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    pub fn serialize<S: Serializer>(u: &Vec<Vec<U256>>, serializer: S) -> Result<S::Ok, S::Error> {
        u.iter()
            .map(|u| u.iter().map(|u| u.to_le_bytes()).collect::<Vec<_>>())
            .collect::<Vec<Vec<[u8; 32]>>>()
            .serialize(serializer)
    }

    #[allow(dead_code)]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Vec<U256>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let u: Vec<Vec<[u8; 32]>> = Deserialize::deserialize(deserializer)?;
        Ok(u.into_iter()
            .map(|i| i.into_iter().map(U256::from_le_bytes).collect())
            .collect())
    }
}

pub(crate) mod vec_fixed_string {
    use std::str::FromStr;

    use alloy_primitives::Address;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };
    use sorella_db_databases::fixed_string::FixedString;

    pub fn serialize<S: Serializer>(u: &Vec<Address>, serializer: S) -> Result<S::Ok, S::Error> {
        u.into_iter()
            .map(|a| format!("{:?}", a).into())
            .collect::<Vec<FixedString>>()
            .serialize(serializer)
    }

    #[allow(dead_code)]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Address>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let addresses: Vec<String> = Deserialize::deserialize(deserializer)?;

        Ok(addresses
            .into_iter()
            .map(|a| Address::from_str(&a))
            .collect::<Result<Vec<_>, _>>()
            .map_err(serde::de::Error::custom)?)
    }
}
pub(crate) mod vec_vec_fixed_string {

    use std::str::FromStr;

    use alloy_primitives::Address;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };
    use sorella_db_databases::fixed_string::FixedString;

    pub fn serialize<S: Serializer>(
        u: &Vec<Vec<Address>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        u.into_iter()
            .map(|addrs| {
                addrs
                    .into_iter()
                    .map(|a| format!("{:?}", a).into())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<Vec<FixedString>>>()
            .serialize(serializer)
    }

    #[allow(dead_code)]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Vec<Address>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let addresses: Vec<Vec<String>> = Deserialize::deserialize(deserializer)?;

        Ok(addresses
            .into_iter()
            .map(|addrs| {
                addrs
                    .into_iter()
                    .map(|a| Address::from_str(&a))
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(serde::de::Error::custom)?)
    }
}

pub(crate) mod vec_b256 {
    use std::str::FromStr;

    use alloy_primitives::B256;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };
    use sorella_db_databases::fixed_string::FixedString;

    pub fn serialize<S: Serializer>(u: &Vec<B256>, serializer: S) -> Result<S::Ok, S::Error> {
        u.into_iter()
            .map(|a| format!("{:?}", a).into())
            .collect::<Vec<FixedString>>()
            .serialize(serializer)
    }

    #[allow(dead_code)]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<B256>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let addresses: Vec<String> = Deserialize::deserialize(deserializer)?;

        Ok(addresses
            .into_iter()
            .map(|a| B256::from_str(&a))
            .collect::<Result<Vec<_>, _>>()
            .map_err(serde::de::Error::custom)?)
    }
}

#[allow(dead_code)]
pub(crate) mod vec_vec_b256 {

    use std::str::FromStr;

    use alloy_primitives::B256;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };
    use sorella_db_databases::fixed_string::FixedString;

    pub fn serialize<S: Serializer>(u: &Vec<Vec<B256>>, serializer: S) -> Result<S::Ok, S::Error> {
        u.into_iter()
            .map(|addrs| {
                addrs
                    .into_iter()
                    .map(|a| format!("{:?}", a).into())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<Vec<FixedString>>>()
            .serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Vec<B256>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let addresses: Vec<Vec<String>> = Deserialize::deserialize(deserializer)?;

        Ok(addresses
            .into_iter()
            .map(|addrs| {
                addrs
                    .into_iter()
                    .map(|a| B256::from_str(&a))
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(serde::de::Error::custom)?)
    }
}
