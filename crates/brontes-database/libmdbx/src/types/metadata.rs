use alloy_primitives::{Address, TxHash, U256};
pub use brontes_types::extra_processing::Pair;
use redefined::RedefinedConvert;
use serde_with::{serde_as, DisplayFromStr};
use sorella_db_databases::{clickhouse, clickhouse::Row};

use super::{
    redefined_types::metadata::Redefined_MetadataInner,
    utils::{option_address, u256},
    LibmdbxData,
};
use crate::{tables::Metadata, CompressedTable};

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

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MetadataInner {
    #[serde(with = "u256")]
    pub block_hash:             U256,
    pub block_timestamp:        u64,
    pub relay_timestamp:        Option<u64>,
    pub p2p_timestamp:          Option<u64>,
    #[serde(with = "option_address")]
    pub proposer_fee_recipient: Option<Address>,
    pub proposer_mev_reward:    Option<u128>,
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub mempool_flow:           Vec<TxHash>,
}
