use serde::{Deserialize, Serialize};
use sorella_db_clients::databases::{clickhouse, clickhouse::Row};

#[derive(Debug, Row, Serialize, Deserialize)]
pub struct DBP2PRelayTimes {
    pub relay_timestamp: u64,
    pub p2p_timestamp: u64,
}

#[derive(Debug, Row, Serialize, Deserialize)]
pub struct DBTokenPrices {
    pub symbol: String,
    pub price1: f64,
    pub price0: f64,
}
