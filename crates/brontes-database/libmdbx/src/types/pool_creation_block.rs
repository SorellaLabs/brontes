use alloy_primitives::Address;
use alloy_rlp::{Decodable, Encodable, RlpDecodable, RlpEncodable};
use brontes_types::{
    impl_compress_decompress_for_encoded_decoded,
};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sorella_db_databases::{clickhouse, Row};

use super::{utils::pools_libmdbx, LibmdbxData};
use crate::tables::{PoolCreationBlocks};

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Row, RlpDecodable, RlpEncodable)]
pub struct PoolCreationBlocksData {
    pub block_number: u64,
    #[serde(with = "pools_libmdbx")]
    pub pools:        PoolsLibmdbx,
}

impl LibmdbxData<PoolCreationBlocks> for PoolCreationBlocksData {
    fn into_key_val(
        &self,
    ) -> (
        <PoolCreationBlocks as reth_db::table::Table>::Key,
        <PoolCreationBlocks as reth_db::table::Table>::Value,
    ) {
        (self.block_number, self.pools.clone())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, RlpDecodable, RlpEncodable)]
pub struct PoolsLibmdbx(pub Vec<Address>);

impl_compress_decompress_for_encoded_decoded!(PoolsLibmdbx);
