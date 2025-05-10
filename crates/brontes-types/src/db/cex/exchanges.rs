use std::default::Default;

use alloy_primitives::Address;
use malachite::{num::conversion::traits::FromSciString, Rational};
use redefined::self_convert_redefined;
use serde::Deserialize;
use strum::Display;

use crate::constants::*;

#[derive(
    Copy,
    Display,
    Debug,
    Clone,
    Default,
    Eq,
    PartialEq,
    Hash,
    serde::Serialize,
    // serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
)]
#[archive_attr(derive(Eq, PartialEq, Hash))]
pub enum CexExchange {
    Binance,
    Bitmex,
    Deribit,
    Okex,
    Coinbase,
    Kraken,
    BybitSpot,
    Kucoin,
    Upbit,
    Huobi,
    GateIo,
    Bitstamp,
    Gemini,
    Average,
    VWAP,
    OptimisticVWAP,
    #[default]
    Unknown,
}

self_convert_redefined!(CexExchange);

impl<'de> serde::Deserialize<'de> for CexExchange {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let cex_exchange: String = Deserialize::deserialize(deserializer)?;
        Ok(cex_exchange.as_str().into())
    }
}

impl CexExchange {
    pub fn to_clickhouse_filter(&self) -> &str {
        match self {
            CexExchange::Binance => "c.exchange = 'binance'",
            CexExchange::Bitmex => "c.exchange = 'bitmex'",
            CexExchange::Deribit => "c.exchange = 'deribit'",
            CexExchange::Okex => "c.exchange = 'okex'",
            CexExchange::Coinbase => "c.exchange = 'coinbase'",
            CexExchange::Kraken => "c.exchange = 'kraken'",
            CexExchange::BybitSpot => "c.exchange = 'bybit'",
            CexExchange::Kucoin => "c.exchange = 'kucoin'",
            CexExchange::Upbit => "c.exchange = 'upbit'",
            CexExchange::Huobi => "c.exchange = 'huobi'",
            CexExchange::GateIo => "c.exchange = 'gate-io",
            CexExchange::Bitstamp => "c.exchange = 'bitstamp'",
            CexExchange::Gemini => "c.exchange = 'gemini'",
            CexExchange::Unknown => "c.exchange = ''",
            CexExchange::Average => "c.exchange = ''",
            CexExchange::VWAP => "c.exchange = ''",
            CexExchange::OptimisticVWAP => "c.exchange = ''",
        }
    }
}

impl From<&str> for CexExchange {
    fn from(value: &str) -> Self {
        let val = value.to_lowercase();
        let value = val.as_str();
        match value {
            "binance" | "binance-futures" => CexExchange::Binance,
            "bitmex" | "Bitmex" => CexExchange::Bitmex,
            "deribit" | "Deribit" => CexExchange::Deribit,
            "okex" | "Okex" | "okex-swap" => CexExchange::Okex,
            "coinbase" | "Coinbase" => CexExchange::Coinbase,
            "kraken" | "Kraken" => CexExchange::Kraken,
            "bybit-spot" | "bybitspot" | "BybitSpot" | "Bybit-Spot" | "Bybit_Spot" | "bybit" => {
                CexExchange::BybitSpot
            }
            "kucoin" | "Kucoin" => CexExchange::Kucoin,
            "upbit" | "Upbit" => CexExchange::Upbit,
            "huobi" | "Huobi" => CexExchange::Huobi,
            "gate-io" | "gateio" | "GateIo" | "Gate_Io" => CexExchange::GateIo,
            "bitstamp" | "Bitstamp" => CexExchange::Bitstamp,
            "gemini" | "Gemini" => CexExchange::Gemini,
            _ => CexExchange::Unknown,
        }
    }
}

pub struct SupportedCexExchanges {
    pub exchanges: Vec<CexExchange>,
}

impl From<Vec<String>> for SupportedCexExchanges {
    fn from(value: Vec<String>) -> Self {
        let exchanges = value
            .iter()
            .map(|val| val.as_str().into())
            .collect::<Vec<CexExchange>>();

        SupportedCexExchanges { exchanges }
    }
}

impl From<String> for CexExchange {
    fn from(value: String) -> Self {
        value.as_str().into()
    }
}

impl CexExchange {
    //TQDO: Add for all supported exchanges
    #[cfg(not(feature = "arbitrum"))]
    pub fn most_common_quote_assets(&self) -> Vec<Address> {
        match self {
            CexExchange::Binance => {
                vec![
                    USDT_ADDRESS,
                    WBTC_ADDRESS,
                    BUSD_ADDRESS,
                    USDC_ADDRESS,
                    BNB_ADDRESS,
                    WETH_ADDRESS,
                    FDUSD_ADDRESS,
                    PAX_DOLLAR_ADDRESS,
                ]
            }
            CexExchange::Bitmex => vec![USDT_ADDRESS, USDC_ADDRESS, WETH_ADDRESS],
            CexExchange::Bitstamp => {
                vec![WBTC_ADDRESS, USDC_ADDRESS, USDT_ADDRESS, PAX_DOLLAR_ADDRESS]
            }
            CexExchange::BybitSpot => {
                vec![USDT_ADDRESS, USDC_ADDRESS, WBTC_ADDRESS, DAI_ADDRESS, WETH_ADDRESS]
            }
            CexExchange::Coinbase => {
                vec![
                    USDC_ADDRESS,
                    USDT_ADDRESS,
                    WBTC_ADDRESS,
                    DAI_ADDRESS,
                    WETH_ADDRESS,
                    DAI_ADDRESS,
                ]
            }
            CexExchange::Deribit => vec![USDT_ADDRESS, USDC_ADDRESS, WBTC_ADDRESS],
            CexExchange::GateIo => vec![USDT_ADDRESS, WETH_ADDRESS, WBTC_ADDRESS, USDC_ADDRESS],
            CexExchange::Gemini => {
                vec![WBTC_ADDRESS, WETH_ADDRESS, GUSD_ADDRESS, DAI_ADDRESS, USDT_ADDRESS]
            }
            CexExchange::Huobi => {
                vec![
                    USDT_ADDRESS,
                    WBTC_ADDRESS,
                    WETH_ADDRESS,
                    HT_ADDRESS,
                    HUSD_ADDRESS,
                    USDC_ADDRESS,
                    USDD_ADDRESS,
                    TUSD_ADDRESS,
                    DAI_ADDRESS,
                    PYUSD_ADDRESS,
                ]
            }
            CexExchange::Kraken => {
                vec![WBTC_ADDRESS, WETH_ADDRESS, USDT_ADDRESS, USDC_ADDRESS, DAI_ADDRESS]
            }
            CexExchange::Kucoin => {
                vec![
                    USDT_ADDRESS,
                    WBTC_ADDRESS,
                    WETH_ADDRESS,
                    USDC_ADDRESS,
                    TUSD_ADDRESS,
                    DAI_ADDRESS,
                ]
            }
            CexExchange::Okex => {
                vec![
                    USDT_ADDRESS,
                    USDC_ADDRESS,
                    WBTC_ADDRESS,
                    WETH_ADDRESS,
                    DAI_ADDRESS,
                    EURT_ADDRESS,
                ]
            }
            CexExchange::Upbit => {
                vec![WETH_ADDRESS, WBTC_ADDRESS, LINK_ADDRESS, EURT_ADDRESS, UNI_TOKEN]
            }
            _ => vec![],
        }
    }

    #[cfg(feature = "arbitrum")]
    pub fn most_common_quote_assets(&self) -> Vec<Address> {
        match self {
            CexExchange::Binance => {
                vec![
                    USDT_ADDRESS,
                    WBTC_ADDRESS,
                    USDC_ADDRESS,
                    WETH_ADDRESS,
                ]
            }
            CexExchange::Bitmex => vec![USDT_ADDRESS, USDC_ADDRESS, WETH_ADDRESS],
            CexExchange::Bitstamp => {
                vec![WBTC_ADDRESS, USDC_ADDRESS, USDT_ADDRESS]
            }
            CexExchange::BybitSpot => {
                vec![USDT_ADDRESS, USDC_ADDRESS, WBTC_ADDRESS, DAI_ADDRESS, WETH_ADDRESS]
            }
            CexExchange::Coinbase => {
                vec![
                    USDC_ADDRESS,
                    USDT_ADDRESS,
                    WBTC_ADDRESS,
                    DAI_ADDRESS,
                    WETH_ADDRESS,
                ]
            }
            CexExchange::Deribit => vec![USDT_ADDRESS, USDC_ADDRESS, WBTC_ADDRESS],
            CexExchange::GateIo => vec![USDT_ADDRESS, WETH_ADDRESS, WBTC_ADDRESS, USDC_ADDRESS],
            CexExchange::Gemini => {
                vec![WBTC_ADDRESS, WETH_ADDRESS, DAI_ADDRESS, USDT_ADDRESS]
            }
            CexExchange::Huobi => {
                vec![
                    USDT_ADDRESS,
                    WBTC_ADDRESS,
                    WETH_ADDRESS,
                    USDC_ADDRESS,
                    DAI_ADDRESS,
                ]
            }
            CexExchange::Kraken => {
                vec![WBTC_ADDRESS, WETH_ADDRESS, USDT_ADDRESS, USDC_ADDRESS, DAI_ADDRESS]
            }
            CexExchange::Kucoin => {
                vec![
                    USDT_ADDRESS,
                    WBTC_ADDRESS,
                    WETH_ADDRESS,
                    USDC_ADDRESS,
                    DAI_ADDRESS,
                ]
            }
            CexExchange::Okex => {
                vec![
                    USDT_ADDRESS,
                    USDC_ADDRESS,
                    WBTC_ADDRESS,
                    WETH_ADDRESS,
                    DAI_ADDRESS,
                ]
            }
            CexExchange::Upbit => {
                vec![WETH_ADDRESS, WBTC_ADDRESS, LINK_ADDRESS]
            }
            _ => vec![],
        }
    }

    /// Returns the maker & taker fees by exchange
    /// Assumes best possible fee structure e.g Binanace VIP 9 for example
    /// Does not account for special market maker rebate programs or special
    /// pairs
    ///
    /// TODO: Account for special fee pairs & stableswap rates
    /// TODO: Account for futures & spot fee deltas
    pub fn fees(&self) -> (Rational, Rational) {
        match self {
            CexExchange::Binance => (
                Rational::from_sci_string("0.00012").unwrap(),
                Rational::from_sci_string("0.00024").unwrap(),
            ),
            CexExchange::Bitmex => (
                Rational::from_sci_string("-0.00025").unwrap(),
                Rational::from_sci_string("0.00075").unwrap(),
            ),
            CexExchange::Deribit => {
                (Rational::from_sci_string("0").unwrap(), Rational::from_sci_string("0").unwrap())
            }
            CexExchange::Okex => (
                Rational::from_sci_string("-0.00005").unwrap(),
                Rational::from_sci_string("0.00015").unwrap(),
            ),
            CexExchange::Coinbase => (
                Rational::from_sci_string("0").unwrap(),
                Rational::from_sci_string("0.0005").unwrap(),
            ),
            CexExchange::Kraken => (
                Rational::from_sci_string("0").unwrap(),
                Rational::from_sci_string("0.001").unwrap(),
            ),
            CexExchange::BybitSpot => (
                Rational::from_sci_string("0.00005").unwrap(),
                Rational::from_sci_string("0.00015").unwrap(),
            ),
            CexExchange::Kucoin => (
                Rational::from_sci_string("-0.00005").unwrap(),
                Rational::from_sci_string("0.00025").unwrap(),
            ),
            CexExchange::Upbit => (
                Rational::from_sci_string("0.0002").unwrap(),
                Rational::from_sci_string("0.0002").unwrap(),
            ),
            CexExchange::Huobi => (
                Rational::from_sci_string("0.000097").unwrap(),
                Rational::from_sci_string("0.000193").unwrap(),
            ),
            CexExchange::GateIo => (
                Rational::from_sci_string("0").unwrap(),
                Rational::from_sci_string("0.0002").unwrap(),
            ),
            CexExchange::Bitstamp => (
                Rational::from_sci_string("0").unwrap(),
                Rational::from_sci_string("0.0003").unwrap(),
            ),
            CexExchange::Gemini => (
                Rational::from_sci_string("0").unwrap(),
                Rational::from_sci_string("0.0003").unwrap(),
            ),
            CexExchange::Average => {
                unreachable!("Cannot get fees for cross exchange average quote")
            }
            CexExchange::Unknown => unreachable!("Unknown cex exchange"),
            CexExchange::VWAP | CexExchange::OptimisticVWAP => {
                unreachable!("Cannot get fees for VWAP")
            }
        }
    }
}
