use std::{fmt, hash::Hash, str::FromStr};

use alloy_primitives::{hex, Address, Bytes as Alloy_Bytes, FixedBytes, Uint};
use derive_more::{Deref, DerefMut, From, Index, IndexMut, IntoIterator};
use redefined::{Redefined, RedefinedConvert};

use crate::pair::Pair;
/*
--------------


UInt



*/
#[derive(
    Debug,
    Clone,
    Copy,
    Eq,
    PartialEq,
    Hash,
    Redefined,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
)]
#[redefined(Uint)]
#[redefined_attr(to_source = "Uint::from_limbs(self.limbs)")]
pub struct Redefined_Uint<const BITS: usize, const LIMBS: usize> {
    #[redefined_attr(func = "src.into_limbs()")]
    limbs: [u64; LIMBS],
}

impl<const BITS: usize, const LIMBS: usize> serde::Serialize for Redefined_Uint<BITS, LIMBS> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let this: Uint<BITS, LIMBS> = self.to_source();
        this.serialize(serializer)
    }
}

impl<'de, const BITS: usize, const LIMBS: usize> serde::Deserialize<'de>
    for Redefined_Uint<BITS, LIMBS>
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let this = Uint::deserialize(deserializer)?;
        Ok(Self::from_source(this))
    }
}

impl<const BITS: usize, const LIMBS: usize> Default for Redefined_Uint<BITS, LIMBS> {
    fn default() -> Self {
        Self { limbs: [0; LIMBS] }
    }
}

pub type Redefined_U256 = Redefined_Uint<256, 4>;
pub type Redefined_U64 = Redefined_Uint<64, 1>;

/*
--------------


FixedBytes



*/
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Deref,
    DerefMut,
    From,
    Index,
    IndexMut,
    IntoIterator,
    Redefined,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
)]
#[archive(compare(PartialEq))]
#[redefined(FixedBytes)]
#[redefined_attr(to_source = "FixedBytes::from_slice(&self.0)")]
pub struct Redefined_FixedBytes<const N: usize>(#[into_iterator(owned, ref, ref_mut)] pub [u8; N]);

impl<const N: usize> serde::Serialize for Redefined_FixedBytes<N> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let this: FixedBytes<N> = self.to_source();
        this.serialize(serializer)
    }
}

impl<'de, const N: usize> serde::Deserialize<'de> for Redefined_FixedBytes<N> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let this = FixedBytes::deserialize(deserializer)?;
        Ok(Self::from_source(this))
    }
}

impl<const N: usize> FromStr for Redefined_FixedBytes<N> {
    type Err = hex::FromHexError;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Redefined_FixedBytes::from_source(FixedBytes::from_str(s)?))
    }
}

impl<const N: usize> fmt::Display for Redefined_FixedBytes<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let this: FixedBytes<N> = self.to_source();
        this.fmt(f)
    }
}

pub type Redefined_TxHash = Redefined_FixedBytes<32>;

/*
--------------


Address



*/
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Deref,
    DerefMut,
    From,
    serde::Serialize,
    serde::Deserialize,
    Index,
    IndexMut,
    IntoIterator,
    Redefined,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
)]
#[archive(compare(PartialEq))]
//#[archive_attr(derive(PartialEq, Eq))]
#[redefined(Address)]
pub struct Redefined_Address(Redefined_FixedBytes<20>);

impl FromStr for Redefined_Address {
    type Err = hex::FromHexError;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Redefined_Address::from_source(Address::from_str(s)?))
    }
}

/*
--------------


Pair



*/

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(Pair)]
pub struct Redefined_Pair(Redefined_Address, Redefined_Address);

impl Hash for ArchivedRedefined_Pair {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let addr0: Redefined_Address =
            rkyv::Deserialize::deserialize(&self.0, &mut rkyv::Infallible).unwrap();
        let addr1: Redefined_Address =
            rkyv::Deserialize::deserialize(&self.1, &mut rkyv::Infallible).unwrap();
        addr0.hash(state);
        addr1.hash(state);
    }
}

impl PartialEq for ArchivedRedefined_Pair {
    fn eq(&self, other: &Self) -> bool {
        let addr0: Redefined_Address =
            rkyv::Deserialize::deserialize(&self.0, &mut rkyv::Infallible).unwrap();
        let addr1: Redefined_Address =
            rkyv::Deserialize::deserialize(&self.1, &mut rkyv::Infallible).unwrap();
        addr0 == other.0 && addr1 == other.1
    }

    fn ne(&self, other: &Self) -> bool {
        !self.eq(other)
    }
}

impl Eq for ArchivedRedefined_Pair {}

/*
--------------


alloy_primitives::Bytes



*/

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(Alloy_Bytes)]
#[redefined_attr(to_source = "self.0.into()", from_source = "Self(src.to_vec())")]
pub struct Redefined_Alloy_Bytes(pub Vec<u8>);
