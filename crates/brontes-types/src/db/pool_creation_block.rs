use alloy_primitives::Address;
use sorella_db_databases::{clickhouse, clickhouse::Row};

#[derive(Debug, Clone, Row, serde::Serialize, serde::Deserialize)]
pub struct PoolsToAddresses(pub Vec<Address>);
