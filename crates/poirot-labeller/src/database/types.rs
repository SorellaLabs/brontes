use clickhouse::Row;
use malachite::Rational;
use serde::Deserialize;

use crate::database::serialize::as_rational;

#[derive(Debug, Clone, Row, Deserialize)]
pub struct DBMempool {
    timestamp: u64,
    tx_hash: String,
    node_id: String,
    source: String,
    vpc: bool,
}

#[derive(Debug, Clone, Row, Deserialize)]
pub struct DBRelays {
    epoch: u32,
    slot: u32,
    timestamp: u64,
    block_number: u32,
    parent_hash: String,
    block_hash: String,
    relay: String,
    builder_name: String,
    builder_pubkey: String,
    fee_recipient: String,
    gas_limit: u64,
    gas_used: u64,
    value: u128,
    tx_num: u16,
}

#[derive(Debug, Clone, Row, Deserialize)]
pub struct DBTardisQuotes {
    exchange: String,
    symbol: String,
    timestamp: u64,
    local_timestamp: u64,
    #[serde(with = "as_rational")]
    ask_amt: Rational,
    #[serde(with = "as_rational")]
    ask_price: Rational,
    #[serde(with = "as_rational")]
    bid_price: Rational,
    #[serde(with = "as_rational")]
    bid_amt: Rational,
}

#[derive(Debug, Clone, Row, Deserialize)]
pub struct DBTardisL2 {
    exchange: String,
    symbol: String,
    timestamp: u64,
    local_timestamp: u64,
    is_snapshot: bool,
    side: String,
    #[serde(with = "as_rational")]
    price: Rational,
    #[serde(with = "as_rational")]
    amt: Rational,
}

#[derive(Debug, Clone, Row, Deserialize)]
pub struct DBTardisTrades {
    exchange: String,
    symbol: String,
    timestamp: u64,
    local_timestamp: u64,
    id: String,
    side: String,
    #[serde(with = "as_rational")]
    price: Rational,
    #[serde(with = "as_rational")]
    amt: Rational,
}
