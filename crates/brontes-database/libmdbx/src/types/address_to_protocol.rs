use std::{default::Default, str::FromStr};

use alloy_rlp::{Decodable, Encodable};
use reth_codecs::{main_codec, Compact};
use reth_db::{
    table::{Compress, Decompress},
    DatabaseError,
};
use reth_primitives::{bytes, Address, BufMut};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sorella_db_databases::{clickhouse, Row};

use super::{utils::pool_tokens, LibmbdxData};
use crate::{clickhouse::serde::address_string, libmdbx::tables::AddressToProtocol};

#[serde_as]
#[derive(Debug, Clone, Row, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AddressToProtocolData {
    #[serde(with = "address_string")]
    address: Address,
    #[serde(with = "pool_tokens")]
    tokens:  StaticBindings,
}

impl LibmbdxData<AddressToProtocol> for AddressToProtocolData {
    fn into_key_val(
        &self,
    ) -> (
        <AddressToProtocol as reth_db::table::Table>::Key,
        <AddressToProtocol as reth_db::table::Table>::Value,
    ) {
        (self.address, self.tokens.clone())
    }
}

#[derive(Debug, Default, PartialEq, Clone, Eq)]
#[main_codec(rlp)]
pub struct PoolTokens2 {
    token0: Address,
    token1: Address,
    token2: Option<Address>,
    token3: Option<Address>,
    token4: Option<Address>,
}

impl IntoIterator for PoolTokens2 {
    type IntoIter = std::vec::IntoIter<Self::Item>;
    type Item = Address;

    fn into_iter(self) -> Self::IntoIter {
        vec![Some(self.token0), Some(self.token1), self.token2, self.token3, self.token4]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .into_iter()
    }
}

impl From<Vec<String>> for PoolTokens2 {
    fn from(value: Vec<String>) -> Self {
        let mut iter = value.into_iter();
        PoolTokens2 {
            token0: Address::from_str(&iter.next().unwrap()).unwrap(),
            token1: Address::from_str(&iter.next().unwrap()).unwrap(),
            token2: iter.next().map(|a| Address::from_str(&a).ok()).flatten(),
            token3: iter.next().map(|a| Address::from_str(&a).ok()).flatten(),
            token4: iter.next().map(|a| Address::from_str(&a).ok()).flatten(),
        }
    }
}

impl Into<Vec<String>> for PoolTokens2 {
    fn into(self) -> Vec<String> {
        vec![Some(self.token0), Some(self.token1), self.token2, self.token3, self.token4]
            .into_iter()
            .map(|addr| addr.map(|a| format!("{:?}", a)))
            .flatten()
            .collect::<Vec<_>>()
    }
}

impl Encodable for PoolTokens2 {
    fn encode(&self, out: &mut dyn BufMut) {
        self.token0.encode(out);
        self.token1.encode(out);
        self.token2.unwrap_or_default().encode(out);
        self.token3.unwrap_or_default().encode(out);
        self.token4.unwrap_or_default().encode(out);
    }
}

impl Decodable for PoolTokens2 {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let mut this = Self {
            token0: Address::decode(buf)?,
            token1: Address::decode(buf)?,
            token2: Some(Address::decode(buf)?),
            token3: Some(Address::decode(buf)?),
            token4: Some(Address::decode(buf)?),
        };

        if this.token2.as_ref().unwrap().is_zero() {
            this.token2 = None;
        }

        if this.token3.as_ref().unwrap().is_zero() {
            this.token3 = None;
        }

        if this.token4.as_ref().unwrap().is_zero() {
            this.token4 = None;
        }

        Ok(this)
    }
}

impl Compress for PoolTokens2 {
    type Compressed = Vec<u8>;

    fn compress_to_buf<B: reth_primitives::bytes::BufMut + AsMut<[u8]>>(self, buf: &mut B) {
        let mut encoded = Vec::new();
        self.encode(&mut encoded);
        buf.put_slice(&encoded);
    }
}

impl Decompress for PoolTokens2 {
    fn decompress<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
        let binding = value.as_ref().to_vec();
        let buf = &mut binding.as_slice();
        Ok(PoolTokens2::decode(buf).map_err(|_| DatabaseError::Decode)?)
    }
}
