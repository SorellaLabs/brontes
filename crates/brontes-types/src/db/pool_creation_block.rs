use alloy_primitives::Address;
use redefined::Redefined;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};
use sorella_db_databases::{clickhouse, clickhouse::Row, database_table, tables::DatabaseTable};

use crate::{db::redefined_types::primitives::*, implement_table_value_codecs_with_zc};

database_table!(PoolCreationBlock, PoolsToAddresses);

#[derive(Debug, Clone, Row, PartialEq, Serialize, Deserialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct PoolsToAddresses(pub Vec<Address>);

implement_table_value_codecs_with_zc!(PoolsToAddressesRedefined);
