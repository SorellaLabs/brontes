use std::str::FromStr;

use brontes_types::libmdbx::serde::address_string;
use redefined::RedefinedConvert;
use reth_primitives::Address;
use serde_with::serde_as;
use sorella_db_databases::clickhouse::{self, Row};

use super::{
    redefined_types::address_to_tokens::Redefined_PoolTokens,
    utils::{address, option_address},
};
use crate::{tables::AddressToTokens, types::utils::pool_tokens, CompressedTable, LibmdbxData};

#[serde_as]
#[derive(Debug, Clone, Row, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AddressToTokensData {
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
        <AddressToTokens as CompressedTable>::DecompressedValue,
    ) {
        (self.address, self.tokens.clone())
    }
}

#[serde_as]
#[derive(Debug, Default, PartialEq, Clone, Eq, serde::Serialize, serde::Deserialize)]
pub struct PoolTokens {
    #[serde(with = "address")]
    pub token0:     Address,
    #[serde(with = "address")]
    pub token1:     Address,
    #[serde(with = "option_address")]
    pub token2:     Option<Address>,
    #[serde(with = "option_address")]
    pub token3:     Option<Address>,
    #[serde(with = "option_address")]
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
