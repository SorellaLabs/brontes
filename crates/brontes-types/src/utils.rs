use alloy_primitives::{I256, U256};
use malachite::{
    num::{
        arithmetic::traits::Pow,
        conversion::traits::{RoundingFrom, RoundingInto},
    },
    rounding_modes::RoundingMode,
    Integer, Natural, Rational,
};

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

// pub fn rational_to_u256_fraction(rational: &Rational) -> ([u8; 32], [u8; 32])
// {     let (num_limbs, denom_limbs) = rational.to_numerator_and_denominator();
//     println!("RATIONAL: {:?}", rational);

//     if let ((num, false), (denom, false)) = (
//         U256::overflowing_from_limbs_slice(&num_limbs.to_limbs_asc()),
//         U256::overflowing_from_limbs_slice(&denom_limbs.to_limbs_asc()),
//     ) {
//         (num.to_le_bytes(), denom.to_le_bytes())
//     } else {
//         let mut simple_rational = rational.clone();
//         simple_rational.mutate_numerator_and_denominator(|n, d| {
//             *n /= Natural::from(10_u8);
//             *d /= Natural::from(10_u8);
//         });

//         rational_to_u256_fraction(&simple_rational)
//     }
// }

// pub fn rational_to_u256_fraction(rational: &Rational) -> ([u8; 32], [u8; 32])
// {     let (num_nat, denom_nat) = rational.to_numerator_and_denominator();
//     println!("RATIONAL: {:?}", rational);

//     if let ((num, false), (denom, false)) = (
//         U256::overflowing_from_limbs_slice(&num_nat.to_limbs_asc()),
//         U256::overflowing_from_limbs_slice(&denom_nat.to_limbs_asc()),
//     ) {
//         (num.to_le_bytes(), denom.to_le_bytes())
//     } else {
//         let u256_max = Natural::from_limbs_asc(U256::MAX.as_limbs());
//         println!("MAX U256: {:?}\n", u256_max);
//         if num_nat > denom_nat {
//             let div_const = Rational::from_naturals_ref(&u256_max, &num_nat);

//             println!("DIV: {:?}", div_const);
//             println!("NOM: {:?}", num_nat);
//             println!("DENOM: {:?}", denom_nat);

//             let rational_denom =
//                 div_const * Rational::from_naturals_ref(&denom_nat,
// &Natural::from(1_u8));             let rounded_denom: Natural =
// rational_denom.rounding_into(RoundingMode::Nearest).0;

//             println!("DENOM ROUND: {:?}", rounded_denom);

//             (
//
// U256::from_limbs_slice(&u256_max.to_limbs_asc()).to_le_bytes(),
// U256::from_limbs_slice(&rounded_denom.to_limbs_asc()).to_le_bytes(),
//             )
//         } else {
//             let div_const = Rational::from_naturals_ref(&u256_max,
// &denom_nat);

//             println!("DIV: {:?}", div_const);
//             println!("DENOM: {:?}", denom_nat);
//             println!("NUM: {:?}", num_nat);

//             let rational_num =
//                 div_const * Rational::from_naturals_ref(&num_nat,
// &Natural::from(1_u8));             let rounded_num: Natural =
// rational_num.rounding_into(RoundingMode::Nearest).0;

//             println!("NUM ROUND: {:?}", rounded_num);

//             // let (final_num, final_denom) =
//             //     Rational::from_naturals(rounded_num,
//             // denom_nat).to_numerator_and_denominator();
//             (
//
// U256::from_limbs_slice(&rounded_num.to_limbs_asc()).to_le_bytes(),
//
// U256::from_limbs_slice(&u256_max.to_limbs_asc()).to_le_bytes(),             )
//         }
//     }
// }

pub fn rational_to_u256_fraction(rational: &Rational) -> ([u8; 32], [u8; 32]) {
    println!("RATIONAL: {:?}", rational);

    let (num_nat, denom_nat) = rational.to_numerator_and_denominator();

    let u256_max = Natural::from_limbs_asc(U256::MAX.as_limbs());

    let num_overflow = num_nat > u256_max;
    let denom_overflow = denom_nat > u256_max;

    if !num_overflow && !denom_overflow {
        let num_u256 = U256::from_limbs_slice(&num_nat.to_limbs_asc());
        let denom_u256 = U256::from_limbs_slice(&denom_nat.to_limbs_asc());
        (num_u256.to_le_bytes(), denom_u256.to_le_bytes())
    } else {
        let scale_factor = if num_overflow && denom_overflow {
            std::cmp::max(num_nat.clone(), denom_nat.clone()) / &u256_max
        } else if num_overflow {
            num_nat.clone() / &u256_max
        } else {
            denom_nat.clone() / &u256_max
        } + Natural::from(1_u8);
        let scaled_num_nat = &num_nat / &scale_factor;
        let scaled_denom_nat = &denom_nat / &scale_factor;

        println!("NUM: {:?}", scaled_num_nat);
        println!("DENOM: {:?}", scaled_denom_nat);

        let scaled_num_u256 = U256::from_limbs_slice(&scaled_num_nat.to_limbs_asc());
        let scaled_denom_u256 = U256::from_limbs_slice(&scaled_denom_nat.to_limbs_asc());

        (scaled_num_u256.to_le_bytes(), scaled_denom_u256.to_le_bytes())
    }
}

// (115792089237316195423570985008687907853269984665640564039457584007913129639935/13445378) - (8006643927551493257961755468982781446000000000000000000000000000000000000000000/941176471)
// (115792089237316195423570985008687907853269984665640564039457584007913129639935/(16000000007*(115792089237316195423570985008687907853269984665640564039457584007913129639935/x))) - (115792089237316195423570985008687907853269984665640564039457584007913129639935/13611295)

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_overflow() {
        let num: u128 = 16000000000;
        let denom: u128 = 16000000007;

        let rational = Rational::from_naturals(Natural::from(num), Natural::from(denom));

        let expected = (U256::from(num).to_le_bytes(), U256::from(denom).to_le_bytes());
        let calculated = rational_to_u256_fraction(&rational);

        assert_eq!(expected, calculated);
    }

    #[test]
    fn test_numerator_overflow() {
        let num: u128 = u128::MAX;
        let denom: u128 = 16000000007;

        let mod_numerator =
            Natural::from(num) * (Natural::from(4_u8) * Natural::from(10_u8).pow(41));
        let mut rational = Rational::from_naturals(mod_numerator.clone(), Natural::from(denom));

        let calculated = rational_to_u256_fraction(&rational);
    }

    #[test]
    fn test_denom_overflow() {
        let num: u128 = 16000000000;
        let denom: u128 = 16000000007;

        let rational = Rational::from_naturals(Natural::from(num), Natural::from(denom));

        let expected = (U256::from(num).to_le_bytes(), U256::from(denom).to_le_bytes());
        let calculated = rational_to_u256_fraction(&rational);

        assert_eq!(expected, calculated);
    }
}
