use alloy_primitives::Address;
use clickhouse::Row;
use redefined::Redefined;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};

use super::cex::CexExchange;
use crate::{
    db::redefined_types::primitives::*,
    implement_table_value_codecs_with_zc,
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
