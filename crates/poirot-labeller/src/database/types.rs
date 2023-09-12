use clickhouse::Row;
use serde::{Deserialize, Serialize};

#[derive(Debug, Row, Serialize, Deserialize)]
pub struct DBP2PRelayTimes {
    pub relay_timestamp: u64,
    pub p2p_timestamp: u64,
}

#[derive(Debug, Row, Serialize, Deserialize)]
pub struct DBTardisTrades {
    pub address0: String,
    pub address1: String,
    pub price1: f64,
    pub price0: f64,
}
