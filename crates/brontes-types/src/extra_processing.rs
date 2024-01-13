use std::str::FromStr;

use alloy_primitives::Address;
use alloy_rlp::{BufMut, Decodable, Encodable};
use reth_codecs::derive_arbitrary;
use reth_db::table::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive_arbitrary(compact)]
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub struct Pair(pub Address, pub Address);

impl Pair {
    pub fn flip(self) -> Self {
        Pair(self.1, self.0)
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
