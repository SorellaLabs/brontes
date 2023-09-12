use malachite::{num::arithmetic::traits::Pow, Natural, Rational};
use reth_primitives::U256;

pub mod normalized_actions;
pub mod structured_trace;
pub mod tree;

include!(concat!(env!("OUT_DIR"), "/token_mapping.rs"));

pub trait ToScaledRational {
    fn to_scaled_rational(self, decimals: u8) -> Rational;
}

impl ToScaledRational for U256 {
    fn to_scaled_rational(self, decimals: u8) -> Rational {
        let top = Natural::from_limbs_desc(&self.into_limbs());

        Rational::from_naturals(top, Natural::from(10u8).pow(decimals as u64))
    }
}
