use clickhouse::Row;
use serde::Deserialize;

#[derive(Debug, Clone, Row, Deserialize)]
pub struct TardisQuotes {
    exchange: String,
    symbol: String,
    timestamp: u64,
    local_timestamp: u64,
    ask_amt: f64,
    ask_price: f64,
    bid_price: f64,
    bid_amt: f64,
}
