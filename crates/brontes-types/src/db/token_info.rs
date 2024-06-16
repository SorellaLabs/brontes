use std::{
    fmt::Display,
    ops::{Deref, DerefMut},
};

use alloy_primitives::Address;
use clickhouse::{DbRow, Row};
use redefined::{self_convert_redefined, Redefined};
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{ser::SerializeStruct, Deserialize, Serialize};

use super::clickhouse_serde::token_info::token_info_des;
use crate::{
    constants::{USDC_ADDRESS, USDT_ADDRESS, WETH_ADDRESS},
    db::redefined_types::primitives::AddressRedefined,
    implement_table_value_codecs_with_zc,
    serde_utils::addresss,
};

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Deserialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct TokenInfoWithAddress {
    #[serde(with = "addresss")]
    pub address: Address,
    #[redefined(same_fields)]
    #[serde(deserialize_with = "token_info_des::deserialize")]
    pub inner:   TokenInfo,
}

impl TokenInfoWithAddress {
    pub fn native_eth() -> Self {
        Self {
            inner:   TokenInfo { decimals: 18, symbol: "ETH".to_string() },
            address: WETH_ADDRESS,
        }
    }

    pub fn weth() -> Self {
        Self {
            inner:   TokenInfo { decimals: 18, symbol: "WETH".to_string() },
            address: WETH_ADDRESS,
        }
    }

    pub fn usdt() -> Self {
        Self {
            inner:   TokenInfo { decimals: 6, symbol: "USDT".to_string() },
            address: USDT_ADDRESS,
        }
    }

    pub fn usdc() -> Self {
        Self {
            inner:   TokenInfo { decimals: 6, symbol: "USDC".to_string() },
            address: USDC_ADDRESS,
        }
    }

    pub fn clickhouse_fmt(&self) -> (String, String) {
        (format!("{:?}", self.address), self.inner.symbol.clone())
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

impl Serialize for TokenInfoWithAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut ser_struct = serializer.serialize_struct("TokenInfoWithAddress", 3)?;

        ser_struct.serialize_field("address", &format!("{:?}", self.address))?;
        ser_struct.serialize_field("symbol", &self.symbol)?;
        ser_struct.serialize_field("decimals", &self.decimals)?;

        ser_struct.end()
    }
}

impl DbRow for TokenInfoWithAddress {
    const COLUMN_NAMES: &'static [&'static str] = &["address", "symbol", "decimals"];
}

#[derive(
    Debug,
    Clone,
    Default,
    Row,
    Deserialize,
    Serialize,
    rSerialize,
    rDeserialize,
    Archive,
    PartialEq,
    Eq,
    Hash,
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
