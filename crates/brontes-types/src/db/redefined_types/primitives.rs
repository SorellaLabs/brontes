use std::{fmt, hash::Hash, str::FromStr};

use alloy_primitives::{hex, Address, Bytes as Alloy_Bytes, FixedBytes, Uint};
use derive_more::{Deref, DerefMut, From, Index, IndexMut, IntoIterator};
use redefined::{redefined_remote, Redefined, RedefinedConvert};
use rkyv::{Archive as rkyvArchive, Deserialize as rkyvDeserialize, Serialize as rkyvSerialize};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::pair::Pair;

// redefined UInt
redefined_remote!(
    #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, rkyvSerialize, rkyvDeserialize, rkyvArchive)]
    Uint : "ruint"
);

impl<const BITS: usize, const LIMBS: usize> Default for UintRedefined<BITS, LIMBS> {
    fn default() -> Self {
        Self { limbs: [0; LIMBS] }
    }
}

pub type U256Redefined = UintRedefined<256, 4>;
pub type U64Redefined = UintRedefined<64, 1>;

/*
--------------


FixedBytes



*/

//#[archive(compare(PartialEq), check_bytes)]
//#[redefined(FixedBytes)]
//#[redefined_attr(to_source = "FixedBytes::from_slice(&self.0)")]

redefined_remote!(
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
        rkyvSerialize,
        rkyvDeserialize,
        rkyvArchive
    )]
    FixedBytes : "alloy-primitives"
);

pub type TxHashRedefined = FixedBytesRedefined<32>;

impl<const N: usize> Serialize for FixedBytesRedefined<N> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let this = self.to_source();
        this.serialize(serializer)
    }
}

impl<'de, const N: usize> Deserialize<'de> for FixedBytesRedefined<N> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let this = FixedBytes::deserialize(deserializer)?;
        Ok(this.into())
    }
}

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
#[archive(check_bytes)]
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
#[archive(check_bytes)]
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
#[archive(check_bytes)]
#[redefined(Alloy_Bytes)]
#[redefined_attr(to_source = "self.0.into()", from_source = "Self(src.to_vec())")]
pub struct Redefined_Alloy_Bytes(pub Vec<u8>);
