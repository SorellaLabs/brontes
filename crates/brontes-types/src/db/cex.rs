//! This module provides structures and functionalities for managing and
//! querying centralized exchange (CEX) price data which is crucial to detect
//! CeFi - Defi arbitrage.
//!
//! ## Data Flow and Storage
//! - Data is initially queried from a ClickHouse database using `brontes init`.
//! - The queried data gets deserialized into `CexPriceMap` struct.
//! - It is then stored in our local libmdbx database in the `cex_price_map`
//!   table.
//!
//! ## Key Components
//! - `CexPriceMap`: A map of CEX prices, organized by exchange and token pairs.
//! - `CexQuote`: Represents an individual price quote from a CEX.
//! - `CexExchange`: Enum of supported CEX exchanges.
use std::{
    default::Default,
    fmt,
    fmt::{Display, Formatter},
    ops::MulAssign,
    str::FromStr,
};

use alloy_primitives::Address;
use clickhouse::Row;
use derive_more::Display;
use malachite::{
    num::{
        arithmetic::traits::ReciprocalAssign, basic::traits::One, conversion::traits::FromSciString,
    },
    Rational,
};
use redefined::{self_convert_redefined, Redefined, RedefinedConvert};
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{ser::SerializeSeq, Deserialize, Serialize};

use super::raw_cex_quotes::RawCexQuotes;
use crate::{
    constants::*,
    db::redefined_types::{malachite::RationalRedefined, primitives::AddressRedefined},
    implement_table_value_codecs_with_zc,
    pair::{Pair, PairRedefined},
    utils::ToFloatNearest,
    FastHashMap,
};

/// Centralized exchange price map organized by exchange.
///
///
/// Each pair is entered into the map with an ordered `Pair` key whereby:
///
/// If: Token0 (base asset) > Token1 (quote asset), then:
///
///  Pair key = (token0, token1)
///
/// Initially when deserializing the clickhouse data we create `CexQuote`
/// entries with token0 as the base asset and token1 as the quote asset.
///
/// This provides us with the actual token0 when the map is queried so we can
/// interpret the price in the correct direction & reciprocate the price (which
/// is a rational) if need be.
#[derive(Debug, Clone, Row, PartialEq, Eq)]
pub struct CexPriceMap(pub FastHashMap<CexExchange, FastHashMap<Pair, CexQuote>>);

#[derive(
    Debug, PartialEq, Clone, serde::Serialize, rSerialize, rDeserialize, Archive, Redefined,
)]
#[redefined(CexPriceMap)]
#[redefined_attr(
    to_source = "CexPriceMap(self.map.into_iter().collect::<FastHashMap<_,_>>().to_source())",
    from_source = "CexPriceMapRedefined::new(src.0)"
)]
pub struct CexPriceMapRedefined {
    pub map: Vec<(CexExchange, FastHashMap<PairRedefined, CexQuoteRedefined>)>,
}

impl CexPriceMapRedefined {
    fn new(map: FastHashMap<CexExchange, FastHashMap<Pair, CexQuote>>) -> Self {
        Self {
            map: map
                .into_iter()
                .map(|(exch, inner_map)| (exch, FastHashMap::from_source(inner_map)))
                .collect::<Vec<_>>(),
        }
    }
}

implement_table_value_codecs_with_zc!(CexPriceMapRedefined);

impl Default for CexPriceMap {
    fn default() -> Self {
        Self::new()
    }
}

impl CexPriceMap {
    pub fn new() -> Self {
        Self(FastHashMap::default())
    }

    /// Retrieves a CEX quote for a specified token pair from a given exchange.
    ///
    /// This function looks up the quote for the `pair` in the context of the
    /// specified `exchange`. The quote is retrieved based on an ordered
    /// `Pair` key, which allows us to avoid duplicate entries for the same
    /// pair.
    ///
    /// ## Parameters
    /// - `pair`: The pair of tokens for which the quote is requested. The pair
    ///   where `pair.0` (token0) is the base asset and `pair.1` (token1) is the
    ///   quote asset.
    /// - `exchange`: The exchange from which to retrieve the quote.
    ///
    /// ## Returns
    /// - Returns `Some(CexQuote)` with the best ask and bid price if a quote is
    ///   found.
    /// - Returns a default `CexQuote` with a 1:1 price ratio if the pair tokens
    ///   are identical.
    /// - If `token0` in the quote differs from `pair.0` parameter, the quote's
    ///   price is reciprocated to match the requested pair ordering.
    pub fn get_quote(&self, pair: &Pair, exchange: &CexExchange) -> Option<CexQuote> {
        if pair.0 == pair.1 {
            return Some(CexQuote { price: (Rational::ONE, Rational::ONE), ..Default::default() })
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

    /// Computes an average quote for a given token pair across multiple
    /// exchanges.
    pub fn get_avg_quote(&self, pair: &Pair, exchanges: &[CexExchange]) -> Option<CexQuote> {
        if pair.0 == pair.1 {
            return Some(CexQuote { price: (Rational::ONE, Rational::ONE), ..Default::default() })
        }

        let ordered_pair = pair.ordered();
        let sum_price = exchanges
            .iter()
            .filter_map(|exchange| self.get_quote(&ordered_pair, exchange))
            .fold((Rational::default(), Rational::default(), 0), |acc, quote| {
                (acc.0 + quote.price.0, acc.1 + quote.price.1, acc.2 + 1)
            });

        if sum_price.2 > 0 {
            let count_rational = Rational::from(sum_price.2);
            Some(CexQuote {
                exchange:  CexExchange::default(),
                timestamp: 0,
                price:     (sum_price.0 / &count_rational, sum_price.1 / count_rational),
                token0:    pair.0,
            })
        } else {
            None
        }
    }

    /// Retrieves a CEX quote for a given token pair using an intermediary
    /// asset.
    ///
    /// This method is used when a direct quote for the pair is not available.
    /// It attempts to construct a quote for `pair` by finding a path
    /// through a common intermediary asset as provided by the `exchange`.
    pub fn get_quote_via_intermediary(
        &self,
        pair: &Pair,
        exchange: &CexExchange,
    ) -> Option<CexQuote> {
        let intermediaries = exchange.most_common_quote_assets();

        println!(
            "Calculating quote for pair {}-{} through intermediaries on {}",
            pair.0, pair.1, exchange
        );

        intermediaries
            .iter()
            .filter_map(|&intermediary| {
                let pair1 = Pair(pair.0, intermediary);
                let pair2 = Pair(intermediary, pair.1);

                println!("Evaluating via intermediary: {}", intermediary);

                if let (Some(quote1), Some(quote2)) =
                    (self.get_quote(&pair1, exchange), self.get_quote(&pair2, exchange))
                {
                    println!("Intermediate quote1 ({}-{}): {}", pair.0, intermediary, quote1);
                    println!("Intermediate quote2 ({}-{}): {}", intermediary, pair.1, quote2);
                    let combined_price =
                        (quote1.price.0 * quote2.price.0, quote1.price.1 * quote2.price.1);
                    let combined_quote = CexQuote {
                        exchange:  *exchange,
                        timestamp: std::cmp::max(quote1.timestamp, quote2.timestamp),
                        price:     combined_price,
                        token0:    pair.0,
                    };

                    println!("Combined quote: {}", combined_quote);

                    Some(combined_quote)
                } else {
                    None
                }
            })
            .max_by(|a, b| a.price.0.cmp(&b.price.0))
            .map(|best_quote| {
                println!("Selected best quote: {}", best_quote);
                best_quote
            })
    }

    /// Retrieves a CEX quote for a given token pair directly or via an
    /// intermediary
    pub fn get_quote_direct_or_via_intermediary(
        &self,
        pair: &Pair,
        exchange: &CexExchange,
    ) -> Option<CexQuote> {
        self.get_quote(pair, exchange)
            .or_else(|| self.get_quote_via_intermediary(pair, exchange))
    }
}

type CexPriceMapDeser = Vec<(String, Vec<((String, String), (u64, (f64, f64), String))>)>;

impl Serialize for CexPriceMap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(None)?;
        for (ex, v) in &self.0 {
            let inner_vec = v
                .iter()
                .map(|(a, b)| {
                    let ordered = a.ordered();
                    (
                        (format!("{}", ordered.0), format!("{}", ordered.1)),
                        (
                            b.timestamp,
                            (b.price.0.clone().to_float(), b.price.1.clone().to_float()),
                            format!("{:?}", b.token0),
                        ),
                    )
                })
                .collect::<Vec<_>>();
            seq.serialize_element(&(ex.to_string(), inner_vec))?;
        }

        seq.end()
    }
}
//TODO: Joe remove the extra string for token_0 it should just be
// base_token_addr
impl<'de> serde::Deserialize<'de> for CexPriceMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let map: CexPriceMapDeser = serde::Deserialize::deserialize(deserializer)?;

        let mut cex_price_map = FastHashMap::default();

        map.into_iter().for_each(|(exchange, meta)| {
            let exchange_map = cex_price_map
                .entry(CexExchange::from(exchange.clone()))
                .or_insert(FastHashMap::default());
            meta.into_iter().for_each(
                |(
                    (base_token_addr, quote_token_addr),
                    (timestamp, (price0, price1), token0_addr),
                )| {
                    exchange_map.insert(
                        Pair(
                            Address::from_str(&base_token_addr).unwrap(),
                            Address::from_str(&quote_token_addr).unwrap(),
                        )
                        .ordered(),
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

/// Represents a price quote from a centralized exchange (CEX).
///
/// `CexQuote` captures the price data for a specific token pair at a given
/// exchange, along with a timestamp indicating when the quote was recorded. The
/// timestamp reflects the exchange's time if available, or the time of
/// recording by [Tardis](https://docs.tardis.dev/downloadable-csv-files#quotes), albeit less commonly.
///
/// ## Fields
/// - `exchange`: The source CEX of the quote.
/// - `timestamp`: The recording time of the quote, closely aligned with the p2p
///   timestamp of block propagation initiation by the proposer.
/// - `price`: A tuple (Rational) representing the best ask and bid prices for
///   the pair.
/// - The best ask price is the lowest price at which a seller is willing to
///   sell the base asset (token0) for the quote asset (token1).
/// - The best bid price is the highest price at which a buyer is willing to buy
///   the base asset (token0) for the quote asset (token1).
///
/// - `token0`: The address of the base asset in the pair.
///
/// ## Context
/// Within `CexPriceMap`, `CexQuote` entries are stored by exchange and an
/// ordered token pair. The ordering ensures `token0` (base asset) is always
/// less than `token1` (quote asset) to avoid duplicate entries and facilitate
/// consistent price interpretation. When queried, if `token0` in `CexQuote`
/// differs from the base asset of the requested pair, the price is reciprocated
/// to align with the actual pair order.
#[derive(Debug, Clone, Default, Row, Eq, serde::Serialize, serde::Deserialize, Redefined)]
#[redefined_attr(derive(
    Debug,
    PartialEq,
    Clone,
    Hash,
    Serialize,
    rSerialize,
    rDeserialize,
    Archive
))]
pub struct CexQuote {
    #[redefined(same_fields)]
    pub exchange:  CexExchange,
    pub timestamp: u64,
    /// Best Ask & Bid price at p2p timestamp (which is when the block is first
    /// propagated by the proposer)
    pub price:     (Rational, Rational),
    pub token0:    Address,
}

impl Display for CexQuote {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Exchange: {}\nTimestamp: {}\nBest Ask Price: {:.2}\nBest Bid Price: {:.2}\nToken \
             Address: {}",
            self.exchange,
            self.timestamp,
            self.price.0.clone().to_float(),
            self.price.1.clone().to_float(),
            self.token0
        )
    }
}

pub struct ExchangeData {
    pub exchange: CexExchange,
    pub quotes:   Vec<CexQuote>,
    pub trades:   Vec<Trade>,
}

pub struct Trade {
    pub exchange:  CexExchange,
    pub timestamp: u64,
    pub price:     Rational,
    pub amount:    Rational,
    pub side:      TradeSide,
}

pub enum TradeSide {
    Buy,
    Sell,
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
            && (self.price.0) == (other.price.0)
            && (self.price.1) == (other.price.1)
    }
}

impl MulAssign for CexQuote {
    fn mul_assign(&mut self, rhs: Self) {
        self.price.0 *= rhs.price.0;
        self.price.1 *= rhs.price.1;
    }
}

impl From<(Pair, RawCexQuotes)> for CexQuote {
    fn from(value: (Pair, RawCexQuotes)) -> Self {
        let (pair, quote) = value;
        let price = if pair == pair.ordered() {
            (
                Rational::try_from_float_simplest(quote.ask_price).unwrap(),
                Rational::try_from_float_simplest(quote.bid_price).unwrap(),
            )
        } else {
            (
                Rational::try_from_float_simplest(1.0 / quote.ask_price).unwrap(),
                Rational::try_from_float_simplest(1.0 / quote.bid_price).unwrap(),
            )
        };

        CexQuote { exchange: quote.exchange, timestamp: quote.timestamp, price, token0: pair.0 }
    }
}

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
    #[default]
    Unknown,
}

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
            CexExchange::Binance => "(exchange = 'binance' or exchange = 'binance-futures')",
            CexExchange::Bitmex => "exchange = 'bitmex'",
            CexExchange::Deribit => "exchange = 'deribit'",
            CexExchange::Okex => "(exchange = 'okex' or exchange = 'okex-swap')",
            CexExchange::Coinbase => "exchange = 'coinbase'",
            CexExchange::Kraken => "exchange = 'kraken'",
            CexExchange::BybitSpot => "(exchange = 'bybit-spot' or exchange = 'bybit')",
            CexExchange::Kucoin => "exchange = 'kucoin'",
            CexExchange::Upbit => "exchange = 'upbit'",
            CexExchange::Huobi => "exchange = 'huobi'",
            CexExchange::GateIo => "exchange = 'gate-io;",
            CexExchange::Bitstamp => "exchange = 'bitstamp'",
            CexExchange::Gemini => "exchange = 'gemini'",
            CexExchange::Unknown => "exchange = ''",
            CexExchange::Average => "exchange = ''",
        }
    }
}

self_convert_redefined!(CexExchange);

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
                    KCS_ADDRESS,
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

    /// Returns the maker & taker fees by exchange
    /// Assumes best possible fee structure e.g Binanace VIP 9 for example
    /// Does not account for special market maker rebate programs
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
        }
    }
}
