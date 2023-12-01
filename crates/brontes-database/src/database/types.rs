use std::str::FromStr;

use alloy_json_abi::JsonAbi;
use reth_primitives::{Address, H160, H256};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use sorella_db_databases::{clickhouse, clickhouse::Row};

#[derive(Debug, Row, Serialize, Deserialize)]
pub struct TokenPricesTimeDB {
    token_prices: Vec<(String, (f64, f64))>,
}

#[derive(Debug, Row, Serialize, Deserialize)]
pub struct DBP2PRelayTimes {
    pub relay_timestamp: u64,
    pub p2p_timestamp:   u64,
}

#[serde_as]
#[derive(Debug, Row, Serialize, Deserialize)]
pub struct DBTokenPrices {
    pub address: H160,
    pub price0:  f64,
    pub price1:  f64,
}

impl From<DBTokenPricesDB> for DBTokenPrices {
    fn from(value: DBTokenPricesDB) -> Self {
        DBTokenPrices {
            address: H160::from_str(&value.address).unwrap(),
            price0:  value.price0,
            price1:  value.price1,
        }
    }
}

#[serde_as]
#[derive(Debug, Row, Serialize, Deserialize)]
pub struct Abis {
    address: String,
    abi:     String,
}

impl From<Abis> for (Address, JsonAbi) {
    fn from(value: Abis) -> Self {
        let address = Address::from_str(&value.address).unwrap();
        let abi = JsonAbi::from_json_str(&value.abi).unwrap();
        (address, abi)
    }
}

#[serde_as]
#[derive(Debug, Row, Serialize, Deserialize)]
pub struct DBTokenPricesDB {
    pub address: String,
    pub price0:  f64,
    pub price1:  f64,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Row)]
pub struct TimesFlowDB {
    pub block_number:    u64,
    pub block_hash:      String,
    pub relay_time:      u64,
    pub p2p_time:        u64,
    pub proposer_addr:   String,
    pub proposer_reward: u64,
    pub private_flow:    Vec<String>,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Row)]
pub struct TimesFlow {
    pub block_number:    u64,
    pub block_hash:      H256,
    pub relay_time:      u64,
    pub p2p_time:        u64,
    pub proposer_addr:   H160,
    pub proposer_reward: u64,
    pub private_flow:    Vec<H256>,
}

impl From<TimesFlowDB> for TimesFlow {
    fn from(value: TimesFlowDB) -> Self {
        TimesFlow {
            block_number:    value.block_number,
            block_hash:      H256::from_str(&value.block_hash).unwrap(),
            relay_time:      value.relay_time,
            p2p_time:        value.p2p_time,
            proposer_addr:   H160::from_str(&value.proposer_addr).unwrap(),
            proposer_reward: value.proposer_reward,
            private_flow:    value
                .private_flow
                .into_iter()
                .map(|tx| H256::from_str(&tx).unwrap())
                .collect(),
        }
    }
}
