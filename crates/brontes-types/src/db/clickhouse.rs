use std::str::FromStr;

use alloy_json_abi::JsonAbi;
use alloy_primitives::{Address, B256, U256};
use clickhouse::{fixed_string::FixedString, Row};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};

use crate::serde_utils::vec_u256;

#[serde_as]
#[derive(Debug, Row, Serialize, Deserialize)]
pub struct ClickhouseAbis {
    address: String,
    abi:     String,
}

impl From<ClickhouseAbis> for (Address, JsonAbi) {
    fn from(value: ClickhouseAbis) -> Self {
        let address = Address::from_str(&value.address).unwrap();
        let abi = JsonAbi::from_json_str(&value.abi).unwrap();
        (address, abi)
    }
}

#[derive(Row, Deserialize, Debug, Clone, PartialEq)]
pub struct ClickhousePoolReserves {
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

#[serde_as]
#[derive(Debug, Row, Serialize, Deserialize)]
pub struct ClickhouseTokenPrices {
    pub key: (String, String),
    pub val: Vec<ClickhouseExchangePrice>,
}

#[serde_as]
#[derive(Debug, Row, Serialize, Deserialize)]
pub struct ClickhouseExchangePrice {
    pub exchange: String,
    /// (base_address, quote_address)
    /// (timestamp, ask_price, bid_price)
    pub val:      (u64, f64, f64),
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, Row)]
pub struct ClickhouseTimesFlow {
    pub block_number:    u64,
    #[serde_as(as = "DisplayFromStr")]
    pub block_hash:      B256,
    pub relay_time:      u64,
    pub p2p_time:        u64,
    #[serde_as(as = "DisplayFromStr")]
    pub proposer_addr:   Address,
    pub proposer_reward: u128,
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub private_flow:    Vec<B256>,
}
