use clickhouse::Row;
use malachite::Rational;
use serde::Deserialize;

use crate::database::serialize::as_rational;

#[derive(Debug, Clone, Row, Deserialize)]
pub struct TardisQuotes {
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
