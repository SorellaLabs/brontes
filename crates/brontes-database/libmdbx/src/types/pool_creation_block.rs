use alloy_primitives::Address;
use serde_with::serde_as;
use sorella_db_databases::clickhouse::{self, Row};

use super::{utils::pools_libmdbx, LibmdbxData};
use crate::{tables::PoolCreationBlocks, CompressedTable};

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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PoolsToAddresses(pub Vec<Address>);
