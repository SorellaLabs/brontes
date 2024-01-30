use brontes_types::db::{
    metadata::MetadataInner,
    redefined_types::primitives::{Redefined_Address, Redefined_TxHash, Redefined_U256},
};
use redefined::{Redefined, RedefinedConvert};
use serde_with::serde_as;
use sorella_db_databases::{clickhouse, clickhouse::Row};

use super::{LibmdbxData, ReturnKV};
use crate::libmdbx::Metadata;

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
#[archive(check_bytes)]
#[redefined(MetadataInner)]
pub struct LibmdbxMetadataInner {
    pub block_hash:             Redefined_U256,
    pub block_timestamp:        u64,
    pub relay_timestamp:        Option<u64>,
    pub p2p_timestamp:          Option<u64>,
    pub proposer_fee_recipient: Option<Redefined_Address>,
    pub proposer_mev_reward:    Option<u128>,
    pub private_flow:           Vec<Redefined_TxHash>,
}
