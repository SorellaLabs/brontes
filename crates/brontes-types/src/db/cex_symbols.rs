use clickhouse::Row;
use serde::Deserialize;

use super::cex::CexExchange;
use crate::{
    pair::Pair,
    serde_utils::{address_pair, cex_exchange},
};

#[derive(Debug, Default, Clone, Row, PartialEq, Deserialize)]
pub struct CexSymbols {
    #[serde(with = "cex_exchange")]
    pub exchange:     CexExchange,
    pub symbol_pair:  String,
    #[serde(with = "address_pair")]
    pub address_pair: Pair,
}
