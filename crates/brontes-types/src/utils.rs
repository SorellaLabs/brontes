use alloy_primitives::{I256, U256};
use eyre::ContextCompat;
use malachite::{
    num::{arithmetic::traits::Pow, conversion::traits::RoundingFrom},
    rounding_modes::RoundingMode,
    Integer, Natural, Rational,
};
use malachite_q::arithmetic::traits::Approximate;

#[allow(unused_imports)]
use crate::{
    display::utils::display_sandwich,
    normalized_actions::{NormalizedBurn, NormalizedLiquidation, NormalizedMint, NormalizedSwap},
    serde_utils::vec_vec_fixed_string,
    GasDetails,
};

pub trait ToScaledRational {
    fn to_scaled_rational(self, decimals: u8) -> Rational;
}

impl ToScaledRational for Rational {
    fn to_scaled_rational(self, decimals: u8) -> Rational {
        self / Rational::from(10usize).pow(decimals as u64)
    }
}

impl ToScaledRational for U256 {
    fn to_scaled_rational(self, decimals: u8) -> Rational {
        let top = Natural::from_limbs_asc(&self.into_limbs());

        Rational::from_naturals(top, Natural::from(10u8).pow(decimals as u64))
    }
}

impl ToScaledRational for I256 {
    fn to_scaled_rational(self, decimals: u8) -> Rational {
        let top = Integer::from_twos_complement_limbs_asc(&self.into_limbs());

        Rational::from_integers(top, Integer::from(10u8).pow(decimals as u64))
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

pub fn rational_to_u256_fraction(rational: &Rational) -> eyre::Result<([u8; 32], [u8; 32])> {
    let (num_nat, denom_nat) = rational.numerator_and_denominator_ref();

    let u256_max =
        Natural::from_limbs_asc((U256::MAX / U256::from(10).pow(U256::from(14))).as_limbs());

    if num_nat <= &u256_max && denom_nat <= &u256_max {
        let num_u256 = U256::from_limbs_slice(&num_nat.to_limbs_asc());
        let denom_u256 = U256::from_limbs_slice(&denom_nat.to_limbs_asc());

        Ok((num_u256.to_le_bytes(), denom_u256.to_le_bytes()))
    } else {
        let approx_rational = rational.approximate(&u256_max);
        let (approx_num, approx_denom) = approx_rational.numerator_and_denominator_ref();

        let num_u256 = U256::checked_from_limbs_slice(&approx_num.to_limbs_asc());
        let denom_u256 = U256::checked_from_limbs_slice(&approx_denom.to_limbs_asc());

        num_u256
            .zip(denom_u256)
            .map(|(n, d)| (n.to_le_bytes(), d.to_le_bytes()))
            .wrap_err(format!("value too big for U256: {:?}", rational))
    }
}

#[cfg(test)]
mod tests {

    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_no_overflow() {
        let num: u128 = 16000000000;
        let denom: u128 = 16000000007;

        let rational = Rational::from_naturals(Natural::from(num), Natural::from(denom));

        let expected = (U256::from(num).to_le_bytes(), U256::from(denom).to_le_bytes());
        let calculated = rational_to_u256_fraction(&rational).unwrap();

        assert_eq!(expected, calculated);
    }

    #[test]
    fn test_large_primes() {
        let num_nat = Natural::from_str("3315792090000000000000000000012345678000000000000000000100000000000000000000000000000000000000000000110000000000000000000000000000000000000000709").unwrap();
        let denom_nat = Natural::from_str("11579209000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000141").unwrap();

        let mut rational = Rational::from_naturals(num_nat, denom_nat);
        let calculated = rational_to_u256_fraction(&rational);
        assert!(calculated.is_ok());

        rational = rational.approximate(&Natural::from_limbs_asc(
            (U256::MAX / U256::from(10).pow(U256::from(14))).as_limbs(),
        ));

        let (num, denom) = rational.to_numerator_and_denominator();

        let expected: eyre::Result<([u8; 32], [u8; 32])> =
            U256::checked_from_limbs_slice(&num.to_limbs_asc())
                .zip(U256::checked_from_limbs_slice(&denom.to_limbs_asc()))
                .map(|(n, d)| (n.to_le_bytes(), d.to_le_bytes()))
                .wrap_err(format!("value too big for U256: {:?}", rational));

        assert!(expected.is_ok());
        assert_eq!(calculated.unwrap(), expected.unwrap())
    }
}
