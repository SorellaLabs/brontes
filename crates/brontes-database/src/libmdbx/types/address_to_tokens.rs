use brontes_types::{
    db::{address_to_tokens::PoolTokens, redefined_types::primitives::Redefined_Address},
    serde_utils::primitives::address_string,
};
use redefined::{Redefined, RedefinedConvert};
use reth_primitives::Address;
use serde_with::serde_as;
use sorella_db_databases::clickhouse::{self, Row};

use super::{CompressedTable, LibmdbxData};
use crate::libmdbx::{types::utils::pool_tokens, AddressToTokens};

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

#[derive(
    Debug,
    PartialEq,
    Clone,
    serde::Serialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(PoolTokens)]
pub struct LibmdbxPoolTokens {
    pub token0:     Redefined_Address,
    pub token1:     Redefined_Address,
    pub token2:     Option<Redefined_Address>,
    pub token3:     Option<Redefined_Address>,
    pub token4:     Option<Redefined_Address>,
    pub init_block: u64,
}
