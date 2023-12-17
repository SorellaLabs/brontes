use std::{collections::HashSet, str::FromStr};

use alloy_json_abi::JsonAbi;
use brontes_types::vec_u256;
use malachite::Rational;
use reth_primitives::{Address, B256, U256};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sorella_db_databases::{
    clickhouse,
    clickhouse::{fixed_string::FixedString, Row},
};

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

#[derive(Row, Deserialize, Debug, Clone, PartialEq)]
pub struct PoolReservesDB {
    pub address:           FixedString,
    pub block_number:      u64,
    pub post_tx_hash:      FixedString,
    #[serde(with = "vec_u256")]
    pub reserves:          Vec<U256>,
    #[serde(rename = "prices.quote_addr")]
    pub prices_quote_addr: Vec<FixedString>,
    #[serde(rename = "prices.base_addr")]
    pub prices_base_addr:  Vec<FixedString>,
    #[serde(rename = "prices.price")]
    pub prices_price:      Vec<f64>,
}

//TODO: (Joe): Fix the query now that I have added support for multiple
// exchanges, thank you

#[serde_as]
#[derive(Debug, Row, Serialize, Deserialize)]
pub struct DBTokenPricesDB {
    pub key: (String, String),
    pub val: Vec<ExchangePriceDB>,
}

#[serde_as]
#[derive(Debug, Row, Serialize, Deserialize)]
pub struct ExchangePriceDB {
    pub exchange: String,
    /// (base_address, quote_address)
    /// (timestamp, ask_price, bid_price)
    pub val:      (u64, f64, f64),
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Row)]
pub struct TimesFlowDB {
    pub block_number:    u32,
    pub block_hash:      String,
    pub relay_time:      u64,
    pub p2p_time:        u64,
    pub proposer_addr:   String,
    pub proposer_reward: u128,
    pub private_flow:    Vec<String>,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Row)]
pub struct TimesFlow {
    pub block_number:    u64,
    pub block_hash:      B256,
    pub relay_time:      u64,
    pub p2p_time:        u64,
    pub proposer_addr:   Address,
    pub proposer_reward: u128,
    pub private_flow:    HashSet<B256>,
}

impl From<TimesFlowDB> for TimesFlow {
    fn from(value: TimesFlowDB) -> Self {
        TimesFlow {
            block_number:    value.block_number as u64,
            block_hash:      B256::from_str(&value.block_hash).unwrap_or_default(),
            relay_time:      value.relay_time,
            p2p_time:        value.p2p_time,
            proposer_addr:   Address::from_str(&value.proposer_addr).unwrap_or_default(),
            proposer_reward: value.proposer_reward,
            private_flow:    value
                .private_flow
                .into_iter()
                .map(|tx| B256::from_str(&tx).unwrap_or_default())
                .collect(),
        }
    }
}
