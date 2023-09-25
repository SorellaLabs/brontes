use reth_primitives::H160;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use sorella_db_clients::databases::{clickhouse, clickhouse::Row};

#[derive(Debug, Row, Serialize, Deserialize)]
pub struct TokenPriceTime {
    pub address: H160,
    pub price:   f64,
}

#[derive(Debug, Row, Serialize, Deserialize)]
pub struct DBP2PRelayTimes {
    pub relay_timestamp: u64,
    pub p2p_timestamp:   u64,
}

#[serde_as]
#[derive(Debug, Row, Serialize, Deserialize)]
pub struct DBTokenPrices {
    #[serde_as(as = "DisplayFromStr")]
    pub address: H160,
    pub price0:  f64,
    pub price1:  f64,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Row)]
pub struct RelayInfo {
    pub relay_time:      u64,
    pub p2p_time:        u64,
    #[serde_as(as = "DisplayFromStr")]
    pub proposer_addr:   H160,
    pub proposer_reward: u64,
}
