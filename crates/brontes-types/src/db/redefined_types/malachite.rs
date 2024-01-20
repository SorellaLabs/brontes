use malachite::{num::basic::traits::Zero, platform_64::Limb, Natural, Rational};
use redefined::{Redefined, RedefinedConvert};
/*
--------------


Rational



*/
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Redefined,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    serde::Serialize,
    serde::Deserialize,
)]
#[redefined(Rational)]
#[redefined_attr(
    to_source = "redefined_to_rational(self)",
    from_source = "rational_to_redefined(src)"
)]
pub struct Redefined_Rational {
    pub sign:        bool,
    pub numerator:   Redefined_Natural,
    pub denominator: Redefined_Natural,
}

fn rational_to_redefined(rational: Rational) -> Redefined_Rational {
    Redefined_Rational {
        sign:        rational >= Rational::ZERO,
        numerator:   Redefined_Natural(Redefined_InnerNatural::from_limbs_asc(
            &rational.to_numerator().to_limbs_asc(),
        )),
        denominator: Redefined_Natural(Redefined_InnerNatural::from_limbs_asc(
            &rational.to_denominator().to_limbs_asc(),
        )),
    }
}

fn redefined_to_rational(rational: Redefined_Rational) -> Rational {
    Rational::from_naturals(
        Natural::from_limbs_asc(&rational.numerator.to_limbs_asc()),
        Natural::from_limbs_asc(&rational.denominator.to_limbs_asc()),
    )
}

/*
--------------


Natural



*/
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Redefined,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    serde::Serialize,
    serde::Deserialize,
)]
#[redefined(Natural)]
#[redefined_attr(to_source = "self.into()", from_source = "src.into()")]
pub struct Redefined_Natural(Redefined_InnerNatural);

impl Redefined_Natural {
    pub fn to_limbs_asc(&self) -> Vec<Limb> {
        match *self {
            Redefined_Natural(Redefined_InnerNatural::Small(0)) => Vec::new(),
            Redefined_Natural(Redefined_InnerNatural::Small(small)) => vec![small],
            Redefined_Natural(Redefined_InnerNatural::Large(ref limbs)) => limbs.clone(),
        }
    }
}

//

/*
--------------


InnerNatural



*/
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum Redefined_InnerNatural {
    Small(Limb),
    Large(Vec<Limb>),
}

impl Redefined_InnerNatural {
    pub fn from_limbs_asc(xs: &[Limb]) -> Redefined_InnerNatural {
        let significant_length = limbs_significant_length(xs);
        match significant_length {
            0 => Redefined_InnerNatural::Small(0),
            1 => Redefined_InnerNatural::Small(xs[0]),
            _ => Redefined_InnerNatural::Large(xs[..significant_length].to_vec()),
        }
    }
}

fn limbs_significant_length(xs: &[Limb]) -> usize {
    xs.iter()
        .enumerate()
        .rev()
        .find(|&(_, &x)| x != 0)
        .map_or(0, |(i, _)| i + 1)
}
