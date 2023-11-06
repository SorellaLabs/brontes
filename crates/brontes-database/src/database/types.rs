use std::str::FromStr;

use reth_primitives::{H160, H256};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use sorella_db_databases::{clickhouse, clickhouse::Row};

#[derive(Debug, Row, Serialize, Deserialize)]
pub struct TokenPriceTime {
    pub address: H160,
    pub price: f64,
}

#[derive(Debug, Row, Serialize, Deserialize)]
pub struct DBP2PRelayTimes {
    pub relay_timestamp: u64,
    pub p2p_timestamp: u64,
}

#[serde_as]
#[derive(Debug, Row, Serialize, Deserialize)]
pub struct DBTokenPrices {
    pub address: H160,
    pub price0: f64,
    pub price1: f64,
}

impl From<DBTokenPricesDB> for DBTokenPrices {
    fn from(value: DBTokenPricesDB) -> Self {
        DBTokenPrices {
            address: H160::from_str(&value.address).unwrap(),
            price0: value.price0,
            price1: value.price1,
        }
    }
}

#[serde_as]
#[derive(Debug, Row, Serialize, Deserialize)]
pub struct DBTokenPricesDB {
    pub address: String,
    pub price0: f64,
    pub price1: f64,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Row)]
pub struct RelayInfoDB {
    pub block_hash: String,
    pub relay_time: u64,
    pub p2p_time: u64,
    pub proposer_addr: String,
    pub proposer_reward: u64,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Row)]
pub struct RelayInfo {
    pub block_hash: H256,
    pub relay_time: u64,
    pub p2p_time: u64,
    pub proposer_addr: H160,
    pub proposer_reward: u64,
}

impl From<RelayInfoDB> for RelayInfo {
    fn from(value: RelayInfoDB) -> Self {
        RelayInfo {
            block_hash: H256::from_str(&value.block_hash).unwrap(),
            relay_time: value.relay_time,
            p2p_time: value.p2p_time,
            proposer_addr: H160::from_str(&value.proposer_addr).unwrap(),
            proposer_reward: value.proposer_reward,
        }
    }
}
