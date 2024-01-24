use brontes_types::db::{
    pool_creation_block::PoolsToAddresses, redefined_types::primitives::Redefined_Address,
};
use redefined::{Redefined, RedefinedConvert};
use serde_with::serde_as;
use sorella_db_databases::clickhouse::{self, Row};

use super::{utils::pools_libmdbx, CompressedTable, LibmdbxData};
use crate::libmdbx::PoolCreationBlocks;

#[serde_as]
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Row)]
pub struct PoolCreationBlocksData {
    pub block_number: u64,
    #[serde(with = "pools_libmdbx")]
    pub pools:        PoolsToAddresses,
}

impl LibmdbxData<PoolCreationBlocks> for PoolCreationBlocksData {
    fn into_key_val(
        &self,
    ) -> (
        <PoolCreationBlocks as reth_db::table::Table>::Key,
        <PoolCreationBlocks as CompressedTable>::DecompressedValue,
    ) {
        (self.block_number, self.pools.clone().into())
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
#[redefined(PoolsToAddresses)]
pub struct LibmdbxPoolsToAddresses(pub Vec<Redefined_Address>);
