use std::{hash::Hash, str::FromStr};

use alloy_primitives::Address;
use alloy_rlp::{BufMut, Decodable, Encodable};
use redefined::Redefined;
use reth_db::table::{Decode, Encode};
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};

use crate::db::redefined_types::primitives::AddressRedefined;

/// Pair has a custom hash impl that will always make sure the pair is ordered
/// before hashing aswell as on equals
#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    Serialize,
    Deserialize,
    PartialOrd,
    Ord,
    Redefined,
    PartialEq,
    Eq,
    Hash,
)]
#[redefined_attr(derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    rDeserialize,
    rSerialize,
    Archive,
))]
#[redefined_attr(other(
    #[archive_attr(derive(Hash, PartialEq, Eq))]
))]
pub struct Pair(pub Address, pub Address);

// impl Hash for Pair {
//     fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
//         let this = self.ordered();
//         this.0.hash(state);
//         this.1.hash(state);
//     }
// }
//
// impl Eq for Pair {}
//
// impl PartialEq for Pair {
//     fn eq(&self, other: &Self) -> bool {
//         self.ordered().0 == other.ordered().0 && self.ordered().1 ==
// other.ordered().1     }
// }

impl Pair {
    pub fn is_zero(&self) -> bool {
        self.0 == Address::ZERO && self.1 == Address::ZERO
    }

    pub fn flip(self) -> Self {
        Pair(self.1, self.0)
    }

    pub fn eq_unordered(&self, other: &Self) -> bool {
        self.0 == other.0 && self.1 == other.1
    }

    pub fn has_base_edge(&self, addr: Address) -> bool {
        self.0 == addr
    }

    pub fn has_quote_edge(&self, addr: Address) -> bool {
        self.1 == addr
    }

    pub fn map_key(addr1: Address, addr2: Address) -> Self {
        if addr1 <= addr2 {
            Pair(addr1, addr2)
        } else {
            Pair(addr2, addr1)
        }
    }

    // Returns an ordered version of the pair
    pub fn ordered(&self) -> Self {
        if self.0 <= self.1 {
            Pair(self.0, self.1)
        } else {
            Pair(self.1, self.0)
        }
    }

    /// returns ordered version as well as if the order changed
    pub fn ordered_changed(&self) -> (bool, Self) {
        if self.0 <= self.1 {
            (false, Pair(self.0, self.1))
        } else {
            (true, Pair(self.1, self.0))
        }
    }
}

impl Encode for Pair {
    type Encoded = [u8; 40];

    fn encode(self) -> Self::Encoded {
        let k0 = self.0.encode();
        let k1 = self.1.encode();
        let mut slice = [0; 40];
        slice[0..20].copy_from_slice(&k0);
        slice[20..].copy_from_slice(&k1);

        slice
    }
}

impl Decode for Pair {
    fn decode<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
        let address0 = &value.as_ref()[0..20];
        let address1 = &value.as_ref()[20..];

        Ok(Pair(Address::from_slice(address0), Address::from_slice(address1)))
    }
}

impl FromStr for Pair {
    type Err = alloy_primitives::AddressError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let addrs = s.split(':').collect::<Vec<_>>();

        let addr0 = addrs[0].parse()?;
        let addr1 = addrs[1].parse()?;
        Ok(Pair(addr0, addr1))
    }
}

impl Encodable for Pair {
    fn encode(&self, out: &mut dyn BufMut) {
        Encodable::encode(&self.0, out);
        Encodable::encode(&self.1, out);
    }
}

impl Decodable for Pair {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let token0 = <Address as Decodable>::decode(buf)?;
        let token1 = <Address as Decodable>::decode(buf)?;

        Ok(Self(token0, token1))
    }
}

#[derive(Debug)]
pub struct ExtraProcessing {
    // decimals that are missing that we want to fill
    pub tokens_decimal_fill: Vec<Address>,
}
