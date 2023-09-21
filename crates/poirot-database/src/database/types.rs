use reth_primitives::Address;
use serde::{Deserialize, Serialize};
use sorella_db_clients::databases::{clickhouse, clickhouse::Row};

#[derive(Debug, Row, Serialize, Deserialize)]
pub struct TokenPriceTime {
    pub address: Address,
    pub price: f64,
}

#[derive(Debug, Row, Serialize, Deserialize)]
pub struct DBP2PRelayTimes {
    pub relay_timestamp: u64,
    pub p2p_timestamp: u64,
}

#[derive(Debug, Row, Serialize, Deserialize)]
pub struct DBTokenPrices {
    pub address: String,
    pub price1: f64,
    pub price0: f64,
}

#[derive(Debug, Clone, Row, Deserialize)]
pub struct RelayInfo {
    pub relay_time: u64,
    pub p2p_time: u64,
    pub proposer_addr: Address,
    pub proposer_reward: u64,
}
