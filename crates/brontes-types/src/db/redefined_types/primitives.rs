use std::{hash::Hash, str::FromStr};

use alloy_primitives::{hex, Address, Bytes, FixedBytes, Uint};
use derive_more::{Deref, DerefMut, From, Index, IndexMut, IntoIterator};
use redefined::{redefined_remote, Redefined, RedefinedConvert};
use rkyv::{
    ser::Serializer as rSerializer, Archive, Archived, Deserialize as rDeserialize, Fallible,
    Serialize as rSerialize, SerializeUnsized,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

// Uint
redefined_remote!(
    #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, rSerialize, rDeserialize, Archive)]
    [Uint] : "ruint"
);

impl<const BITS: usize, const LIMBS: usize> Serialize for UintRedefined<BITS, LIMBS> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let this: Uint<BITS, LIMBS> = (*self).into();
        this.serialize(serializer)
    }
}

pub type U256Redefined = UintRedefined<256, 4>;
pub type U64Redefined = UintRedefined<64, 1>;

//FixedBytes
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
        rSerialize,
        rDeserialize,
        Archive,
    )]
    [FixedBytes] : "alloy-primitives"
);

pub type TxHashRedefined = FixedBytesRedefined<32>;
pub type B256Redefined = FixedBytesRedefined<32>;

impl<const N: usize> Serialize for FixedBytesRedefined<N> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let this = self.to_source();
        Serialize::serialize(&this, serializer)
    }
}

impl<'de, const N: usize> Deserialize<'de> for FixedBytesRedefined<N> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let this: FixedBytes<N> = Deserialize::deserialize(deserializer)?;
        Ok(this.into())
    }
}

impl<const N: usize> Hash for ArchivedFixedBytesRedefined<N> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<const N: usize> Eq for ArchivedFixedBytesRedefined<N> {}

impl<const N: usize> PartialEq for ArchivedFixedBytesRedefined<N> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

/// Address
/// Haven't implemented macro stuff yet
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
    Serialize,
    Deserialize,
    Index,
    IndexMut,
    IntoIterator,
    Redefined,
    rSerialize,
    rDeserialize,
    Archive,
)]
#[redefined(Address)]
#[archive_attr(derive(Hash, PartialEq, Eq))]
pub struct AddressRedefined(FixedBytesRedefined<20>);

impl FromStr for AddressRedefined {
    type Err = hex::FromHexError;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(AddressRedefined::from_source(Address::from_str(s)?))
    }
}

/// alloy_primitivies::Bytes
/// Have not implements parsing 'Bytes::bytes' yet
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, Deserialize, Default, Redefined)]
#[redefined(Bytes)]
#[redefined_attr(transmute)]
pub struct BytesRedefined(pub bytes::Bytes);

type ArchivedBytesRedefined = Vec<u8>;

pub struct ResolverForBytesRedefined;

impl Archive for BytesRedefined {
    type Archived = ArchivedBytesRedefined;
    type Resolver = ResolverForBytesRedefined;

    unsafe fn resolve(&self, _: usize, _: Self::Resolver, out: *mut Self::Archived) {
        *out = self.0.to_vec();
    }
}

impl<S: rkyv::ser::Serializer + ?Sized + rkyv::ser::ScratchSpace> rkyv::Serialize<S>
    for BytesRedefined
{
    fn serialize(&self, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
        self.0.as_ref().serialize_unsized(serializer)?;
        Ok(ResolverForBytesRedefined)
    }
}

impl<D: Fallible + ?Sized> rDeserialize<BytesRedefined, D> for Archived<BytesRedefined> {
    fn deserialize(&self, _: &mut D) -> Result<BytesRedefined, D::Error> {
        Ok(BytesRedefined(bytes::Bytes::from(self.clone())))
    }
}
