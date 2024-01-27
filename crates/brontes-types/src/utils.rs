use std::fmt::Debug;

use alloy_primitives::U256;
use malachite::{
    num::{arithmetic::traits::Pow, conversion::traits::RoundingFrom},
    rounding_modes::RoundingMode,
    Natural, Rational,
};
use redefined::{self_convert_redefined, RedefinedConvert};
use serde_repr::{Deserialize_repr, Serialize_repr};
use strum::EnumIter;

#[allow(unused_imports)]
use crate::{
    display::utils::{display_sandwich, print_mev_type_header},
    normalized_actions::{NormalizedBurn, NormalizedLiquidation, NormalizedMint, NormalizedSwap},
    serde_primitives::vec_fixed_string,
    GasDetails,
};

#[derive(
    Debug,
    Serialize_repr,
    Deserialize_repr,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    PartialEq,
    Eq,
    Hash,
    EnumIter,
    Clone,
    Copy,
)]
#[repr(u8)]
#[allow(non_camel_case_types)]
#[serde(rename_all = "lowercase")]
pub enum PriceKind {
    Cex = 0,
    Dex = 1,
}

self_convert_redefined!(PriceKind);

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
