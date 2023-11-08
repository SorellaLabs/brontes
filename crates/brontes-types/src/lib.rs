use malachite::{
    num::{arithmetic::traits::Pow, conversion::traits::RoundingFrom},
    rounding_modes::RoundingMode,
    Natural, Rational,
};
use reth_primitives::U256;

pub mod classified_mev;
pub mod normalized_actions;
pub mod structured_trace;
pub mod tree;

#[cfg(feature = "tests")]
pub mod test_utils;

include!(concat!(env!("ABI_BUILD_DIR"), "/token_mapping.rs"));

pub trait ToScaledRational {
    fn to_scaled_rational(self, decimals: u8) -> Rational;
}

impl ToScaledRational for U256 {
    fn to_scaled_rational(self, decimals: u8) -> Rational {
        let top = Natural::from_limbs_asc(&self.into_limbs());

        Rational::from_naturals(top, Natural::from(10u8).pow(decimals as u64))
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

pub(crate) mod u256 {
    use reth_primitives::U256;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    pub fn serialize<S: Serializer>(u: &U256, serializer: S) -> Result<S::Ok, S::Error> {
        let mut buf: [u8; 32] = u.to_le_bytes();
        buf.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<U256, D::Error>
    where
        D: Deserializer<'de>,
    {
        let u: [u8; 32] = Deserialize::deserialize(deserializer)?;
        Ok(U256::from_le_bytes(u))
    }

    pub fn serialize_vec<S: Serializer>(u: &Vec<U256>, serializer: S) -> Result<S::Ok, S::Error> {
        u.iter()
            .map(|u| u.to_le_bytes())
            .collect::<Vec<[u8; 32]>>()
            .serialize(serializer)
    }

    pub fn deserialize_vec<'de, D>(deserializer: D) -> Result<Vec<U256>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let u: Vec<[u8; 32]> = Deserialize::deserialize(deserializer)?;
        Ok(u.into_iter().map(U256::from_le_bytes).collect())
    }

    pub fn serialize_vec_vec<S: Serializer>(
        u: &Vec<Vec<U256>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        u.iter()
            .map(|u| u.iter().map(|u| u.to_le_bytes()).collect::<Vec<_>>())
            .collect::<Vec<Vec<[u8; 32]>>>()
            .serialize(serializer)
    }

    pub fn deserialize_vec_vec<'de, D>(deserializer: D) -> Result<Vec<Vec<U256>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let u: Vec<Vec<[u8; 32]>> = Deserialize::deserialize(deserializer)?;
        Ok(u.into_iter()
            .map(|i| i.into_iter().map(U256::from_le_bytes).collect())
            .collect())
    }
}
