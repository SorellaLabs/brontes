use std::collections::HashMap;

use brontes_pricing::{
    PoolPairInfoDirection, PoolPairInformation, Protocol, SubGraphEdge, SubGraphsEntry,
};
use brontes_types::{db::redefined_types::primitives::Redefined_Address, pair::Pair};
use redefined::{Redefined, RedefinedConvert};
use sorella_db_databases::clickhouse::{self, Row};

use super::{LibmdbxData, ReturnKV};
use crate::libmdbx::SubGraphs;

// #[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Row)]
// pub struct SubGraphsData {
//     pub pair: Pair,
//     pub data: SubGraphsEntry,
// }
//
// impl LibmdbxData<SubGraphs> for SubGraphsData {
//     fn into_key_val(&self) -> ReturnKV<SubGraphs> {
//         (self.pair, self.data.clone()).into()
//     }
// }

#[derive(
    Debug,
    Clone,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Redefined,
)]
#[archive(check_bytes)]
#[redefined(SubGraphsEntry)]
pub struct LibmdbxSubGraphsEntry(pub HashMap<u64, Vec<LibmdbxSubGraphEdge>>);

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Redefined,
)]
#[archive(check_bytes)]
#[redefined(PoolPairInformation)]
pub struct LibmdbxPoolPairInformation {
    pub pool_addr: Redefined_Address,
    pub dex_type:  Protocol,
    pub token_0:   Redefined_Address,
    pub token_1:   Redefined_Address,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Redefined,
)]
#[archive(check_bytes)]
#[redefined(PoolPairInfoDirection)]
pub struct LibmdbxPoolPairInfoDirection {
    pub info:       LibmdbxPoolPairInformation,
    pub token_0_in: bool,
}

#[derive(
    Debug,
    Clone,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Redefined,
)]
#[archive(check_bytes)]
#[redefined(SubGraphEdge)]
pub struct LibmdbxSubGraphEdge {
    pub info:                   LibmdbxPoolPairInfoDirection,
    pub distance_to_start_node: u8,
    pub distance_to_end_node:   u8,
}
