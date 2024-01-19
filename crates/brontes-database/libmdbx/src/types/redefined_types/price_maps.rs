use std::hash::Hash;

use brontes_database::Pair;
use brontes_types::libmdbx::redefined_types::primitives::Redefined_Address;
use redefined::{Redefined, RedefinedConvert};
use rkyv::Deserialize;

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
        let addr0: Redefined_Address = self.0.deserialize(&mut rkyv::Infallible).unwrap();
        let addr1: Redefined_Address = self.1.deserialize(&mut rkyv::Infallible).unwrap();
        addr0.hash(state);
        addr1.hash(state);
    }
}

impl PartialEq for ArchivedRedefined_Pair {
    fn eq(&self, other: &Self) -> bool {
        let addr0: Redefined_Address = self.0.deserialize(&mut rkyv::Infallible).unwrap();
        let addr1: Redefined_Address = self.1.deserialize(&mut rkyv::Infallible).unwrap();
        addr0 == other.0 && addr1 == other.1
    }

    fn ne(&self, other: &Self) -> bool {
        !self.eq(other)
    }
}

impl Eq for ArchivedRedefined_Pair {}
