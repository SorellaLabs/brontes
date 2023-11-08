#![feature(trait_alias)]
#![feature(trivial_bounds)]
#![feature(const_type_id)]
#![feature(core_intrinsics)]
#![feature(const_for)]
#![feature(const_mut_refs)]
#![allow(internal_features)]
#![allow(clippy::type_complexity)]

pub mod multi_block;
pub use multi_block::*;
pub mod buf_writer;
pub mod db_write_trigger;
pub mod test_limiter;
pub use test_limiter::*;
pub mod hasher;
pub mod rayon_utils;
pub use hasher::*;
pub use rayon_utils::*;
pub mod action_iter;
pub use action_iter::*;
pub mod executor;
pub use executor::*;
pub mod constants;
pub mod db;
pub mod display;
pub mod mev;
pub mod normalized_actions;
pub mod pair;
pub mod price_graph_types;
pub use price_graph_types::*;
pub mod queries;
pub mod serde_utils;
pub mod unordered_buffer_map;
pub mod unzip_either;
pub use queries::make_call_request;
pub mod structured_trace;
pub mod traits;
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
}
