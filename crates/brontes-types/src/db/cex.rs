use std::{
    collections::HashMap,
    default::{self, Default},
    ops::MulAssign,
    str::FromStr,
};

use alloy_primitives::Address;
use malachite::{
    num::{
        arithmetic::traits::{Floor, ReciprocalAssign},
        basic::traits::One,
    },
    Rational,
};
use sorella_db_databases::clickhouse::{self, Row};

use super::clickhouse::ClickhouseTokenPrices;
use crate::{constants::*, pair::Pair};

/// Each pair is entered into the map with the addresses in order by value:
/// Ergo if token0 < token1, then the pair is (token0, token1)
/// So when we query the map we order the addresses in the pair and then query
/// the quote provides us with the actual token0 so we can interpret the price
/// in any direction
#[derive(Debug, Clone, Row, PartialEq, Eq, serde::Serialize)]
pub struct CexPriceMap(pub HashMap<CexExchange, HashMap<Pair, CexQuote>>);

impl<'de> serde::Deserialize<'de> for CexPriceMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let map: Vec<(String, Vec<((String, String), (u64, (f64, f64), String))>)> =
            serde::Deserialize::deserialize(deserializer)?;

        let mut cex_price_map = HashMap::new();
        map.into_iter().for_each(|(exchange, meta)| {
            let exchange_map = cex_price_map
                .entry(CexExchange::from(exchange.clone()))
                .or_insert(HashMap::new());
            meta.into_iter().for_each(
                |(
                    (base_token_addr, quote_token_addr),
                    (timestamp, (price0, price1), token0_addr),
                )| {
                    exchange_map.insert(
                        Pair(
                            Address::from_str(&base_token_addr).unwrap(),
                            Address::from_str(&quote_token_addr).unwrap(),
                        ),
                        CexQuote {
                            exchange: CexExchange::from(exchange.clone()),
                            timestamp,
                            price: (
                                Rational::try_from_float_simplest(price0).unwrap(),
                                Rational::try_from_float_simplest(price1).unwrap(),
                            ),
                            token0: Address::from_str(&token0_addr).unwrap(),
                        },
                    );
                },
            );
        });

        Ok(CexPriceMap(cex_price_map))
    }
}

impl Default for CexPriceMap {
    fn default() -> Self {
        Self::new()
    }
}

impl CexPriceMap {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn wrap(map: HashMap<CexExchange, HashMap<Pair, CexQuote>>) -> Self {
        Self(map)
    }

    pub fn get_quote(&self, pair: &Pair, exchange: &CexExchange) -> Option<CexQuote> {
        if pair.0 == pair.1 {
            return Some(CexQuote { price: (Rational::ONE, Rational::ONE), ..Default::default() });
        }

        self.0
            .get(exchange)
            .and_then(|quotes| quotes.get(&pair.ordered()))
            .map(|quote| {
                if quote.token0 == pair.0 {
                    quote.clone()
                } else {
                    let mut reciprocal_quote = quote.clone();
                    reciprocal_quote.inverse_price();
                    reciprocal_quote
                }
            })
    }

    pub fn get_binance_quote(&self, pair: &Pair) -> Option<CexQuote> {
        self.get_quote(pair, &CexExchange::Binance)
    }

    pub fn get_avg_quote(&self, pair: &Pair, exchanges: &[CexExchange]) -> Option<CexQuote> {
        if pair.0 == pair.1 {
            return Some(CexQuote { price: (Rational::ONE, Rational::ONE), ..Default::default() });
        }

        let ordered_pair = pair.ordered();
        let mut sum_price = (Rational::default(), Rational::default());
        let mut count = 0;

        for exchange in exchanges {
            if let Some(quotes) = self.0.get(exchange) {
                if let Some(quote) = quotes.get(&ordered_pair) {
                    let adjusted_quote = if quote.token0 == pair.0 {
                        quote.price.clone()
                    } else {
                        let (num, denom) = quote.price.clone();
                        (denom, num) // Invert price
                    };
                    sum_price.0 += adjusted_quote.0;
                    sum_price.1 += adjusted_quote.1;
                    count += 1;
                }
            }
        }

        if count > 0 {
            let count_rational = Rational::from(count);
            Some(CexQuote {
                exchange:  CexExchange::default(),
                timestamp: 0,
                price:     (sum_price.0 / count_rational.clone(), sum_price.1 / count_rational),
                token0:    pair.0,
            })
        } else {
            None
        }
    }
}

/*
impl From<Vec<ClickhouseTokenPrices>> for CexPriceMap {
    fn from(value: Vec<ClickhouseTokenPrices>) -> Self {
        let mut map: HashMap<Pair, Vec<CexQuote>> = HashMap::new();

        for token_info in value {
            let pair = Pair::map_key(
                Address::from_str(&token_info.key.0).unwrap(),
                Address::from_str(&token_info.key.1).unwrap(),
            );

            let quotes: Vec<CexQuote> = token_info
                .val
                .into_iter()
                .map(|exchange_price| CexQuote {
                    exchange:  exchange_price.exchange.into(),
                    timestamp: exchange_price.val.0,
                    price:     (
                        Rational::try_from(exchange_price.val.1).unwrap(),
                        Rational::try_from(exchange_price.val.2).unwrap(),
                    ),
                    token0:    Address::from_str(&token_info.key.0).unwrap(),
                })
                .collect();

            map.insert(pair, quotes);
        }

        CexPriceMap(map)
    }
}*/

#[derive(Debug, Clone, Default, Row, Eq, serde::Serialize, serde::Deserialize)]
pub struct CexQuote {
    pub exchange:  CexExchange,
    pub timestamp: u64,
    /// Best Ask & Bid price at p2p timestamp (which is when the block is first
    /// propagated by the relay / proposer)
    pub price:     (Rational, Rational),
    pub token0:    Address,
}

impl CexQuote {
    fn inverse_price(&mut self) {
        self.price.0.reciprocal_assign();
        self.price.1.reciprocal_assign();
    }
}
impl CexQuote {
    pub fn avg(&self) -> Rational {
        (&self.price.0 + &self.price.1) / Rational::from(2)
    }

    pub fn best_ask(&self) -> Rational {
        self.price.0.clone()
    }

    pub fn best_bid(&self) -> Rational {
        self.price.1.clone()
    }
}

impl PartialEq for CexQuote {
    fn eq(&self, other: &Self) -> bool {
        self.timestamp == other.timestamp
            && (self.price.0.clone() * Rational::try_from(1000000000).unwrap()).floor()
                == (other.price.0.clone() * Rational::try_from(1000000000).unwrap()).floor()
            && (self.price.1.clone() * Rational::try_from(1000000000).unwrap()).floor()
                == (other.price.1.clone() * Rational::try_from(1000000000).unwrap()).floor()
    }
}

impl MulAssign for CexQuote {
    fn mul_assign(&mut self, rhs: Self) {
        self.price.0 *= rhs.price.0;
        self.price.1 *= rhs.price.1;
    }
}

#[derive(Debug, Clone, Default, Eq, serde::Serialize, serde::Deserialize, PartialEq, Hash)]
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
    #[default]
    Unknown,
}

impl From<&str> for CexExchange {
    fn from(value: &str) -> Self {
        match value {
            "binance" | "Binance" => CexExchange::Binance,
            "bitmex" | "Bitmex" => CexExchange::Bitmex,
            "deribit" | "Deribit" => CexExchange::Deribit,
            "okex" | "Okex" => CexExchange::Okex,
            "coinbase" | "Coinbase" => CexExchange::Coinbase,
            "kraken" | "Kraken" => CexExchange::Kraken,
            "bybit-spot" | "bybitspot" | "BybitSpot" => CexExchange::BybitSpot,
            "kucoin" | "Kucoin" => CexExchange::Kucoin,
            "upbit" | "Upbit" => CexExchange::Upbit,
            "huobi" | "Huobi" => CexExchange::Huobi,
            "gate-io" | "gateio" | "GateIo" => CexExchange::GateIo,
            "bitstamp" | "Bitstamp" => CexExchange::Bitstamp,
            "gemini" | "Gemini" => CexExchange::Gemini,
            _ => CexExchange::Unknown,
        }
    }
}

impl From<String> for CexExchange {
    fn from(value: String) -> Self {
        value.as_str().into()
    }
}

impl CexExchange {
    /// Returns the maker & taker fees by exchange
    /// Assumes best possible fee structure e.g Binanace VIP 9 for example
    /// Does not account for special market maker rebate programs
    pub fn fees(&self) -> (Rational, Rational) {
        match self {
            CexExchange::Binance => {
                (Rational::from_str("0.00012").unwrap(), Rational::from_str("0.00024").unwrap())
            }
            CexExchange::Bitmex => {
                (Rational::from_str("-0.00025").unwrap(), Rational::from_str("0.00075").unwrap())
            }
            CexExchange::Deribit => {
                (Rational::from_str("0").unwrap(), Rational::from_str("0").unwrap())
            }
            CexExchange::Okex => {
                (Rational::from_str("-0.00005").unwrap(), Rational::from_str("0.00015").unwrap())
            }
            CexExchange::Coinbase => {
                (Rational::from_str("0").unwrap(), Rational::from_str("0.0005").unwrap())
            }
            CexExchange::Kraken => {
                (Rational::from_str("0").unwrap(), Rational::from_str("0.001").unwrap())
            }
            CexExchange::BybitSpot => {
                (Rational::from_str("0.00005").unwrap(), Rational::from_str("0.00015").unwrap())
            }
            CexExchange::Kucoin => {
                (Rational::from_str("-0.00005").unwrap(), Rational::from_str("0.00025").unwrap())
            }
            CexExchange::Upbit => {
                (Rational::from_str("0.0002").unwrap(), Rational::from_str("0.0002").unwrap())
            }
            CexExchange::Huobi => {
                (Rational::from_str("0.000097").unwrap(), Rational::from_str("0.000193").unwrap())
            }
            CexExchange::GateIo => {
                (Rational::from_str("0").unwrap(), Rational::from_str("0.0002").unwrap())
            }
            CexExchange::Bitstamp => {
                (Rational::from_str("0").unwrap(), Rational::from_str("0.0003").unwrap())
            }
            CexExchange::Gemini => {
                (Rational::from_str("0").unwrap(), Rational::from_str("0.0003").unwrap())
            }
            CexExchange::Unknown => unreachable!(),
        }
    }
}
