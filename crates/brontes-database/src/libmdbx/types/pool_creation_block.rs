use brontes_types::db::{
    pool_creation_block::PoolsToAddresses, redefined_types::primitives::Redefined_Address,
};
use redefined::{Redefined, RedefinedConvert};
use serde_with::serde_as;
use sorella_db_databases::clickhouse::{self, Row};

use super::{utils::pools_libmdbx, LibmdbxData, ReturnKV};
use crate::libmdbx::PoolCreationBlocks;

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
#[redefined(PoolsToAddresses)]
pub struct LibmdbxPoolsToAddresses(pub Vec<Redefined_Address>);
