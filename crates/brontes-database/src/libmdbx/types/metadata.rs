use brontes_types::db::{
    metadata::MetadataInner,
    redefined_types::primitives::{Redefined_Address, Redefined_TxHash, Redefined_U256},
};
use redefined::{Redefined, RedefinedConvert};
use serde_with::serde_as;
use sorella_db_databases::{clickhouse, clickhouse::Row};

use super::{CompressedTable, LibmdbxData};
use crate::libmdbx::Metadata;

#[serde_as]
#[derive(Debug, Clone, Row, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MetadataData {
    pub block_number: u64,
    pub inner:        MetadataInner,
}

impl LibmdbxData<Metadata> for MetadataData {
    fn into_key_val(
        &self,
    ) -> (<Metadata as reth_db::table::Table>::Key, <Metadata as CompressedTable>::DecompressedValue)
    {
        (self.block_number, self.inner.clone())
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
#[redefined(MetadataInner)]
pub struct LibmdbxMetadataInner {
    pub block_hash:             Redefined_U256,
    pub block_timestamp:        u64,
    pub relay_timestamp:        Option<u64>,
    pub p2p_timestamp:          Option<u64>,
    pub proposer_fee_recipient: Option<Redefined_Address>,
    pub proposer_mev_reward:    Option<u128>,
    pub mempool_flow:           Vec<Redefined_TxHash>,
}
