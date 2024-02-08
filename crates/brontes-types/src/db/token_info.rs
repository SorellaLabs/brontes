use std::{
    fmt::Display,
    ops::{Deref, DerefMut},
};

use alloy_primitives::Address;
use redefined::{self_convert_redefined, Redefined};
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};
use sorella_db_databases::{clickhouse, clickhouse::Row};

use crate::{
    constants::{USDT_ADDRESS, WETH_ADDRESS},
    db::redefined_types::primitives::AddressRedefined,
    implement_table_value_codecs_with_zc,
};

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct TokenInfoWithAddress {
    #[redefined(same_fields)]
    pub inner:   TokenInfo,
    pub address: Address,
}

impl TokenInfoWithAddress {
    pub fn native_eth() -> Self {
        Self {
            inner:   TokenInfo { decimals: 18, symbol: "WETH".to_string() },
            address: WETH_ADDRESS.into(),
        }
    }

    pub fn usdt() -> Self {
        Self {
            inner:   TokenInfo { decimals: 6, symbol: "USDT".to_string() },
            address: USDT_ADDRESS.into(),
        }
    }
}

impl Display for TokenInfoWithAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "symbol: {}", self.inner.symbol)
    }
}

impl Deref for TokenInfoWithAddress {
    type Target = TokenInfo;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for TokenInfoWithAddress {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[derive(
    Debug, Clone, Default, Row, Serialize, rSerialize, rDeserialize, Archive, PartialEq, Eq, Hash,
)]
pub struct TokenInfo {
    pub decimals: u8,
    pub symbol:   String,
}

impl TokenInfo {
    pub fn new(decimals: u8, symbol: String) -> Self {
        Self { symbol, decimals }
    }
}

self_convert_redefined!(TokenInfo);
implement_table_value_codecs_with_zc!(TokenInfo);

impl<'de> serde::Deserialize<'de> for TokenInfo {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let val: (u8, String) = serde::Deserialize::deserialize(deserializer)?;

        Ok(Self { decimals: val.0, symbol: val.1 })
    }
}
