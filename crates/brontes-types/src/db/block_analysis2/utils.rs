use std::{collections::HashMap, hash::Hash, str::FromStr};

use alloy_primitives::Address;
use clickhouse::Row;
use itertools::Itertools;
use reth_primitives::TxHash;
use serde::{de::Visitor, Deserialize, Serialize};
use serde_with::serde_as;

use crate::{
    db::{searcher::Fund, token_info::TokenInfoWithAddress},
    mev::{Bundle, BundleData, Mev, MevBlock, MevType},
    pair::Pair,
    serde_utils::{option_fund, option_protocol, option_txhash, vec_fund, vec_protocol},
    Protocol,
};

pub trait FourOptionUnzip<A, B, C, D> {
    fn four_unzip(self) -> (Option<A>, Option<B>, Option<C>, Option<D>)
    where
        Self: Sized;
}

impl<A, B, C, D> FourOptionUnzip<A, B, C, D> for Option<(A, B, C, D)> {
    fn four_unzip(self) -> (Option<A>, Option<B>, Option<C>, Option<D>)
    where
        Self: Sized,
    {
        self.map(|i| (Some(i.0), Some(i.1), Some(i.2), Some(i.3)))
            .unwrap_or_default()
    }
}

pub trait TupleTwoVecUnzip<A, B, C, D> {
    fn four_unzip(self) -> (Vec<A>, Vec<B>, Vec<C>, Vec<D>)
    where
        Self: Sized;
}

impl<A, B, C, D> TupleTwoVecUnzip<A, B, C, D> for (Vec<(A, B)>, Vec<(C, D)>) {
    fn four_unzip(self) -> (Vec<A>, Vec<B>, Vec<C>, Vec<D>)
    where
        Self: Sized,
    {
        let (a, b) = self.0.into_iter().unzip();
        let (c, d) = self.1.into_iter().unzip();
        (a, b, c, d)
    }
}

#[derive(Default, Debug, Clone, Hash, PartialEq, Eq)]
pub struct TokenPairDetails {
    pub address0: Address,
    pub symbol0:  String,
    pub address1: Address,
    pub symbol1:  String,
}

impl Serialize for TokenPairDetails {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        (
            (format!("{:?}", self.address0), self.symbol0.clone()),
            (format!("{:?}", self.address1), self.symbol1.clone()),
        )
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TokenPairDetails {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        let ((token0_address, token0_symbol), (token1_address, token1_symbol)): (
            (String, String),
            (String, String),
        ) = serde::Deserialize::deserialize(deserializer)?;

        Ok(Self {
            address0: Address::from_str(&token0_address).unwrap_or_default(),
            symbol0:  token0_symbol,
            address1: Address::from_str(&token1_address).unwrap_or_default(),
            symbol1:  token1_symbol,
        })
    }
}

impl From<(TokenInfoWithAddress, TokenInfoWithAddress)> for TokenPairDetails {
    fn from(value: (TokenInfoWithAddress, TokenInfoWithAddress)) -> Self {
        let (token0, token1) = if Pair(value.0.address, value.1.address).is_ordered() {
            value
        } else {
            (value.1, value.0)
        };

        Self {
            address0: token0.address,
            symbol0:  token0.symbol.clone(),
            address1: token1.address,
            symbol1:  token1.symbol.clone(),
        }
    }
}

#[derive(Default, Debug, Clone, Hash, PartialEq, Eq)]
pub struct SingleTokenDetails {
    pub address: Address,
    pub symbol:  String,
}

impl Serialize for SingleTokenDetails {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        (format!("{:?}", self.address), self.symbol.clone()).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SingleTokenDetails {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        let (address, symbol): (String, String) = serde::Deserialize::deserialize(deserializer)?;

        Ok(Self { address: Address::from_str(&address).unwrap_or_default(), symbol })
    }
}

impl From<TokenInfoWithAddress> for SingleTokenDetails {
    fn from(value: TokenInfoWithAddress) -> Self {
        Self { address: value.address, symbol: value.inner.symbol }
    }
}
