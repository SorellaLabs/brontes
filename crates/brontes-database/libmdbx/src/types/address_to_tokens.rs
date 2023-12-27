use std::{default::Default, str::FromStr};

use alloy_rlp::{Decodable, Encodable};
use brontes_types::libmdbx::serde::address_string;
use reth_codecs::{main_codec, Compact};
use reth_db::{
    table::{Compress, Decompress},
    DatabaseError,
};
use reth_primitives::{bytes, Address, BufMut};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sorella_db_databases::{clickhouse, Row};

use crate::{tables::AddressToTokens, types::utils::pool_tokens, LibmdbxData};

#[serde_as]
#[derive(Debug, Clone, Row, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AddressToTokensData {
    #[serde(with = "address_string")]
    pub address: Address,
    #[serde(with = "pool_tokens")]
    pub tokens:  PoolTokens,
}

impl LibmdbxData<AddressToTokens> for AddressToTokensData {
    fn into_key_val(
        &self,
    ) -> (
        <AddressToTokens as reth_db::table::Table>::Key,
        <AddressToTokens as reth_db::table::Table>::Value,
    ) {
        (self.address, self.tokens.clone())
    }
}

#[derive(Debug, Default, PartialEq, Clone, Eq)]
#[main_codec(rlp)]
pub struct PoolTokens {
    pub token0:     Address,
    pub token1:     Address,
    pub token2:     Option<Address>,
    pub token3:     Option<Address>,
    pub token4:     Option<Address>,
    pub init_block: u64,
}

impl IntoIterator for PoolTokens {
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

impl From<(Vec<String>, u64)> for PoolTokens {
    fn from(value: (Vec<String>, u64)) -> Self {
        let init_block = value.1;
        let value = value.0;
        let mut iter = value.into_iter();
        PoolTokens {
            token0: Address::from_str(&iter.next().unwrap()).unwrap(),
            token1: Address::from_str(&iter.next().unwrap()).unwrap(),
            token2: iter.next().and_then(|a| Address::from_str(&a).ok()),
            token3: iter.next().and_then(|a| Address::from_str(&a).ok()),
            token4: iter.next().and_then(|a| Address::from_str(&a).ok()),
            init_block,
        }
    }
}

impl From<PoolTokens> for Vec<String> {
    fn from(val: PoolTokens) -> Self {
        vec![Some(val.token0), Some(val.token1), val.token2, val.token3, val.token4]
            .into_iter()
            .filter_map(|addr| addr.map(|a| format!("{:?}", a)))
            .collect::<Vec<_>>()
    }
}

impl Encodable for PoolTokens {
    fn encode(&self, out: &mut dyn BufMut) {
        self.token0.encode(out);
        self.token1.encode(out);
        self.token2.unwrap_or_default().encode(out);
        self.token3.unwrap_or_default().encode(out);
        self.token4.unwrap_or_default().encode(out);
        self.init_block.encode(out);
    }
}

impl Decodable for PoolTokens {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let mut this = Self {
            token0:     Address::decode(buf)?,
            token1:     Address::decode(buf)?,
            token2:     Some(Address::decode(buf)?),
            token3:     Some(Address::decode(buf)?),
            token4:     Some(Address::decode(buf)?),
            init_block: u64::decode(buf)?,
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

impl Compress for PoolTokens {
    type Compressed = Vec<u8>;

    fn compress_to_buf<B: reth_primitives::bytes::BufMut + AsMut<[u8]>>(self, buf: &mut B) {
        let mut encoded = Vec::new();
        self.encode(&mut encoded);
        buf.put_slice(&encoded);
    }
}

impl Decompress for PoolTokens {
    fn decompress<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
        let binding = value.as_ref().to_vec();
        let buf = &mut binding.as_slice();
        PoolTokens::decode(buf).map_err(|_| DatabaseError::Decode)
    }
}
