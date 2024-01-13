use std::str::FromStr;

use alloy_primitives::Address;
use alloy_rlp::{BufMut, Decodable, Encodable};
use reth_codecs::derive_arbitrary;
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
        self.0.encode(out);
        self.1.encode(out);
    }
}

impl Decodable for Pair {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let token0 = Address::decode(buf)?;
        let token1 = Address::decode(buf)?;

        Ok(Self(token0, token1))
    }
}

#[derive(Debug)]
pub struct ExtraProcessing {
    // decimals that are missing that we want to fill
    pub tokens_decimal_fill: Vec<Address>,
}
