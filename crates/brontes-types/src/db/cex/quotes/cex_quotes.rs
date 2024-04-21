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
    cmp::{max, min},
    default::Default,
    fmt,
    fmt::{Display, Formatter},
    ops::MulAssign,
    //str::FromStr,
};

use alloy_primitives::Address;
use clickhouse::Row;
use colored::*;
use itertools::Itertools;
use malachite::{
    num::{
        arithmetic::traits::Reciprocal,
        basic::traits::{One, Two, Zero},
        conversion::traits::FromSciString,
    },
    Rational,
};
use redefined::{self_convert_redefined, Redefined, RedefinedConvert};
use reth_primitives::TxHash;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
#[allow(unused_imports)]
use serde::{ser::SerializeSeq, Deserialize, Serialize};
use tracing::error;

use super::raw_cex_quotes::RawCexQuotes;
use crate::{
    constants::*,
    db::{
        cex::CexExchange,
        redefined_types::{malachite::RationalRedefined, primitives::AddressRedefined},
    },
    implement_table_value_codecs_with_zc,
    normalized_actions::NormalizedSwap,
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
/// is stored as a malachite rational) if need be.
#[derive(Debug, Clone, Row, PartialEq, Eq)]
pub struct CexPriceMap(pub FastHashMap<CexExchange, FastHashMap<Pair, Vec<CexQuote>>>);

#[derive(
    Debug, PartialEq, Clone, serde::Serialize, rSerialize, rDeserialize, Archive, Redefined,
)]
#[redefined(CexPriceMap)]
#[redefined_attr(
    to_source = "CexPriceMap(self.map.into_iter().collect::<FastHashMap<_,_>>().to_source())",
    from_source = "CexPriceMapRedefined::new(src.0)"
)]
pub struct CexPriceMapRedefined {
    pub map: Vec<(CexExchange, FastHashMap<PairRedefined, Vec<CexQuoteRedefined>>)>,
}

impl CexPriceMapRedefined {
    fn new(map: FastHashMap<CexExchange, FastHashMap<Pair, Vec<CexQuote>>>) -> Self {
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

    /// Retrieves a volume weighted CEX quote for a specified token pair from a
    /// given exchange.
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
    pub fn get_quote(&self, pair: &Pair, exchange: &CexExchange) -> Option<FeeAdjustedQuote> {
        if pair.0 == pair.1 {
            return Some(FeeAdjustedQuote {
                price_maker: (Rational::ONE, Rational::ONE),
                price_taker: (Rational::ONE, Rational::ONE),
                ..Default::default()
            })
        }

        self.0
            .get(exchange)
            .and_then(|quotes| quotes.get(&pair.ordered()))
            .map(|quotes| {
                let flip = quotes[0].token0 != pair.0;

                let mut cumulative_bbo = (Rational::ZERO, Rational::ZERO);
                let mut volume_price = (Rational::ZERO, Rational::ZERO);

                for quote in quotes {
                    if flip {
                        let true_quote = quote.inverse_price();
                        cumulative_bbo.0 += &true_quote.amount.0;
                        cumulative_bbo.1 += &true_quote.amount.1;

                        volume_price.0 += &true_quote.price.0 * &true_quote.amount.0;
                        volume_price.1 += &true_quote.price.1 * &true_quote.amount.1;
                    } else {
                        cumulative_bbo.0 += &quote.amount.0;
                        cumulative_bbo.1 += &quote.amount.1;

                        volume_price.0 += &quote.price.0 * &quote.amount.0;
                        volume_price.1 += &quote.price.1 * &quote.amount.1;
                    }
                }

                let volume_weighted_bid = volume_price.0 / &cumulative_bbo.0;
                let volume_weighted_ask = volume_price.1 / &cumulative_bbo.1;

                let fees = exchange.fees();

                let fee_adjusted_maker = (
                    &volume_weighted_bid * (Rational::ONE - &fees.0),
                    &volume_weighted_ask * (Rational::ONE - &fees.0),
                );

                let fee_adjusted_taker = (
                    volume_weighted_bid * (Rational::ONE - &fees.1),
                    volume_weighted_ask * (Rational::ONE - &fees.1),
                );

                FeeAdjustedQuote {
                    exchange:    *exchange,
                    timestamp:   quotes[0].timestamp,
                    price_maker: (fee_adjusted_maker.0, fee_adjusted_maker.1),
                    price_taker: (fee_adjusted_taker.0, fee_adjusted_taker.1),
                    token0:      pair.0,
                    // This is the sum of bid and ask amounts for each quote in this time
                    // window, exchange & pair. This does not represent the total amount available
                    amount:      (cumulative_bbo.0, cumulative_bbo.1),
                }
            })
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
        dex_swap: Option<&NormalizedSwap>,
        tx_hash: Option<&TxHash>,
    ) -> Option<FeeAdjustedQuote> {
        let intermediaries = exchange.most_common_quote_assets();

        intermediaries
            .iter()
            .filter_map(|&intermediary| {
                let pair1 = Pair(intermediary, pair.1);
                let pair2 = Pair(pair.0, intermediary);

                if let (Some(quote1), Some(quote2)) =
                    (self.get_quote(&pair1, exchange), self.get_quote(&pair2, exchange))
                {
                    let combined_price_maker = (
                        &quote1.price_maker.0 * &quote2.price_maker.0,
                        &quote1.price_maker.1 * &quote2.price_maker.1,
                    );

                    let combined_price_taker = (
                        &quote1.price_taker.0 * &quote2.price_taker.0,
                        &quote1.price_taker.1 * &quote2.price_taker.1,
                    );

                    let normalized_bbo_amount = (
                        (&quote1.price_maker.0 * &quote1.amount.0)
                            + (&quote2.price_maker.0 * &quote2.amount.0),
                        (&quote1.price_maker.1 * &quote1.amount.1)
                            + (&quote2.price_maker.1 * &quote2.amount.1),
                    );

                    let combined_quote = FeeAdjustedQuote {
                        exchange:    *exchange,
                        timestamp:   std::cmp::max(quote1.timestamp, quote2.timestamp),
                        price_maker: combined_price_maker,
                        price_taker: combined_price_taker,
                        token0:      pair.1,
                        amount:      normalized_bbo_amount,
                    };

                    if let Some(swap) = dex_swap {
                        let swap_rate = swap.swap_rate();
                        let smaller = min(&swap_rate, &combined_quote.price_maker.1);
                        let larger = max(&swap_rate, &combined_quote.price_maker.1);

                        if smaller * Rational::TWO < *larger {
                            log_significant_price_difference(
                                swap,
                                exchange,
                                &combined_quote,
                                &quote1,
                                &quote2,
                                &intermediary.to_string(),
                                tx_hash,
                            );
                            None
                        } else {
                            Some(combined_quote)
                        }
                    } else {
                        Some(combined_quote)
                    }
                } else {
                    None
                }
            })
            .max_by(|a, b| a.amount.0.cmp(&b.amount.0))
    }

    /// Retrieves a CEX quote for a given token pair directly or via an
    /// intermediary
    pub fn get_quote_direct_or_via_intermediary(
        &self,
        pair: &Pair,
        exchange: &CexExchange,
        dex_swap: Option<&NormalizedSwap>,
        tx_hash: Option<&TxHash>,
    ) -> Option<FeeAdjustedQuote> {
        self.get_quote(pair, exchange)
            .or_else(|| self.get_quote_via_intermediary(pair, exchange, dex_swap, tx_hash))
    }

    pub fn get_volume_weighted_quote(
        &self,
        exchange_quotes: &[&FeeAdjustedQuote],
        dex_swap: &NormalizedSwap,
    ) -> Option<FeeAdjustedQuote> {
        let mut cumulative_bbo = (Rational::ZERO, Rational::ZERO);
        let mut vw_price_maker = (Rational::ZERO, Rational::ZERO);
        let mut vw_price_taker = (Rational::ZERO, Rational::ZERO);

        let mut avg_timestamp = 0;

        for quote in exchange_quotes {
            cumulative_bbo.0 += &quote.amount.0;
            cumulative_bbo.1 += &quote.amount.1;

            vw_price_maker.0 += &quote.price_maker.0 * &quote.amount.0;
            vw_price_maker.1 += &quote.price_maker.1 * &quote.amount.1;

            vw_price_taker.0 += &quote.price_taker.0 * &quote.amount.0;
            vw_price_taker.1 += &quote.price_taker.1 * &quote.amount.1;

            avg_timestamp += quote.timestamp;
        }

        let volume_weighted_bid_maker = vw_price_maker.0 / &cumulative_bbo.0;
        let volume_weighted_ask_maker = vw_price_maker.1 / &cumulative_bbo.1;

        let volume_weighted_bid_taker = vw_price_taker.0 / &cumulative_bbo.0;
        let volume_weighted_ask_taker = vw_price_taker.1 / &cumulative_bbo.1;

        let avg_timestamp = avg_timestamp / exchange_quotes.len() as u64;
        let avg_amount = (
            &cumulative_bbo.0 / Rational::from(exchange_quotes.len()),
            &cumulative_bbo.1 / Rational::from(exchange_quotes.len()),
        );

        let smaller = dex_swap.swap_rate().min(volume_weighted_ask_maker.clone());
        let larger = dex_swap.swap_rate().max(volume_weighted_ask_maker.clone());

        if smaller * Rational::from(2) < larger {
            log_significant_cross_exchange_vmap_difference(
                exchange_quotes
                    .iter()
                    .map(|q| q.exchange.to_string())
                    .collect_vec()
                    .join(", "),
                dex_swap,
                volume_weighted_ask_maker,
                &dex_swap.token_out.address,
                &dex_swap.token_in.address,
            );
            None
        } else {
            Some(FeeAdjustedQuote {
                exchange:    CexExchange::VWAP,
                timestamp:   avg_timestamp,
                price_maker: (volume_weighted_bid_maker, volume_weighted_ask_maker),
                price_taker: (volume_weighted_bid_taker, volume_weighted_ask_taker),
                token0:      exchange_quotes[0].token0,
                amount:      avg_amount,
            })
        }
    }

    pub fn get_binance_quote(&self, pair: &Pair) -> Option<FeeAdjustedQuote> {
        self.get_quote(pair, &CexExchange::Binance)
    }
}

fn log_significant_price_difference(
    dex_swap: &NormalizedSwap,
    exchange: &CexExchange,
    combined_quote: &FeeAdjustedQuote,
    quote1: &FeeAdjustedQuote,
    quote2: &FeeAdjustedQuote,
    intermediary: &str,
    tx_hash: Option<&TxHash>,
) {
    error!(
        "   \n\x1b[1;31mSignificant price difference detected for {} - {} on {}:\x1b[0m\n\
                - \x1b[1;34mDEX Swap Rate:\x1b[0m {:.6}\n\
                - \x1b[1;34mCEX Combined Quote:\x1b[0m {:.6}\n\
                    * First Leg Price: {:.7}\n\
                    * Second Leg Price: {:.7}\n\
                - Token Contracts:\n\
                    * Token Out: https://etherscan.io/address/{}\n\
                    * Intermediary: https://etherscan.io/address/{}\n\
                    * Token In: https://etherscan.io/address/{}\n\
                {}",
        dex_swap.token_out_symbol(),
        dex_swap.token_in_symbol(),
        exchange,
        dex_swap.swap_rate().to_float(),
        combined_quote.price_maker.1.clone().to_float(),
        quote1.price_maker.1.clone().to_float(),
        quote2.price_maker.1.clone().to_float(),
        dex_swap.token_out.address,
        intermediary,
        dex_swap.token_in.address,
        tx_hash.map_or(String::new(), |hash| format!("- Transaction Hash: https://etherscan.io/tx/{}", hash))
    );
}

fn log_significant_cross_exchange_vmap_difference(
    exchange_list: String,
    dex_swap: &NormalizedSwap,
    vmap_quote: Rational,
    token_out_address: &Address,
    token_in_address: &Address,
) {
    error!(
        "   \n\x1b[1;31mSignificant price difference in cross exchange VMAP detected for {} - {} on VWAP:\x1b[0m\n\
                - \x1b[1;34mDEX Swap Rate:\x1b[0m {:.6}\n\
                - \x1b[1;34mCEX VMAP Quote:\x1b[0m {:.6}\n\
                - \x1b[1;34mExchanges:\x1b[0m {}\n\
                - Token Contracts:\n\
                * Token Out: https://etherscan.io/address/{}\n\
                * Token In: https://etherscan.io/address/{}",
        dex_swap.token_out_symbol(),
        dex_swap.token_in_symbol(),
        dex_swap.swap_rate().to_float(),
        vmap_quote.to_float(),
        exchange_list,
        token_out_address,
        token_in_address,
    );
}

#[allow(dead_code)]
type CexPriceMapDeser =
    Vec<(String, Vec<((String, String), (u64, (f64, f64), (f64, f64), String))>)>;

#[allow(dead_code)]
impl Serialize for CexPriceMap {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        /*
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
                            (b.amount.0.clone().to_float(), b.amount.1.clone().to_float()),
                            format!("{:?}", b.token0),
                        ),
                    )
                })
                .collect::<Vec<_>>();
            seq.serialize_element(&(ex.to_string(), inner_vec))?;
        }

        seq.end()
        */
        todo!()
    }
}

#[allow(dead_code)]
impl<'de> serde::Deserialize<'de> for CexPriceMap {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        /*
        let map: CexPriceMapDeser = serde::Deserialize::deserialize(deserializer)?;

        let mut cex_price_map = FastHashMap::default();

        map.into_iter().for_each(|(exchange, meta)| {
            let exchange_map = cex_price_map
                .entry(CexExchange::from(exchange.clone()))
                .or_insert(FastHashMap::default());
            meta.into_iter().for_each(
                |(
                    (base_token_addr, quote_token_addr),
                    (timestamp, (price0, price1), (amt0, amt1), token0_addr),
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
                            amount: (
                                Rational::try_from_float_simplest(amt0).unwrap(),
                                Rational::try_from_float_simplest(amt1).unwrap(),
                            ),
                        },
                    );
                },
            );
        });

        Ok(CexPriceMap(cex_price_map))
        */
        Ok(Self::default())
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
    /// Best Bid & Ask price
    pub price:     (Rational, Rational),
    /// Bid & Ask amount
    pub amount:    (Rational, Rational),
    pub token0:    Address,
}

impl Display for CexQuote {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Exchange: {}\nTimestamp: {}\nBest Ask Price: {:.2}\nBest Bid Price: {:.2}\nBase 
             \
             Asset Address: https://etherscan.io/address/{}",
            self.exchange,
            self.timestamp,
            self.price.0.clone().to_float(),
            self.price.1.clone().to_float(),
            self.token0
        )
    }
}

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
pub struct FeeAdjustedQuote {
    #[redefined(same_fields)]
    pub exchange:    CexExchange,
    pub timestamp:   u64,
    /// Best fee adjusted Bid & Ask price (maker)
    pub price_maker: (Rational, Rational),
    /// Best fee adjusted Bid & Ask price (taker)
    pub price_taker: (Rational, Rational),
    /// Bid & Ask amount
    pub amount:      (Rational, Rational),
    pub token0:      Address,
}

impl fmt::Display for FeeAdjustedQuote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let price_maker = self.price_maker.clone();
        let price_taker = self.price_taker.clone();
        let amount = self.amount.clone();

        writeln!(f, "{}", "Fee Adjusted Quote:".bold().underline())?;
        writeln!(f, "   Exchange: {}", self.exchange)?;
        writeln!(f, "   Timestamp: {}", self.timestamp)?;
        writeln!(f, "   Prices:")?;
        writeln!(
            f,
            "       Maker: Bid {:.4}, Ask {:.4}",
            price_maker.0.to_float(),
            price_maker.1.to_float()
        )?;
        writeln!(
            f,
            "       Taker: Bid {:.4}, Ask {:.4}",
            price_taker.0.to_float(),
            price_taker.1.to_float()
        )?;
        writeln!(f, "   Amounts:")?;
        writeln!(f, "       Bid Amount: {:.4}", amount.0.to_float())?;
        writeln!(f, "       Ask Amount: {:.4}", amount.1.to_float())?;
        writeln!(f, "       Token0: {}", self.token0)?;

        Ok(())
    }
}
impl FeeAdjustedQuote {
    pub fn maker_taker_mid(&self) -> (Rational, Rational) {
        (
            (&self.price_maker.0 + &self.price_maker.1) / Rational::TWO,
            (&self.price_taker.0 + &self.price_taker.1) / Rational::TWO,
        )
    }

    pub fn maker_taker_ask(self) -> (Rational, Rational) {
        (self.price_maker.1, self.price_taker.1)
    }
}

impl PartialEq for FeeAdjustedQuote {
    fn eq(&self, other: &Self) -> bool {
        self.timestamp == other.timestamp
            && (self.price_maker.0) == (other.price_maker.0)
            && (self.price_maker.1) == (other.price_maker.1)
    }
}

impl MulAssign for FeeAdjustedQuote {
    fn mul_assign(&mut self, rhs: Self) {
        self.price_maker.0 *= rhs.price_maker.0;
        self.price_maker.1 *= rhs.price_maker.1;
        self.price_taker.0 *= rhs.price_taker.0;
        self.price_taker.1 *= rhs.price_taker.1;
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
    fn inverse_price(&self) -> Self {
        Self {
            exchange:  self.exchange,
            timestamp: self.timestamp,
            price:     (self.price.1.clone().reciprocal(), self.price.0.clone().reciprocal()),
            token0:    self.token0,
            amount:    (
                &self.amount.1 * self.price.1.clone().reciprocal(),
                &self.amount.0 * self.price.0.clone().reciprocal(),
            ),
        }
    }

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

        let price = (
            Rational::try_from_float_simplest(quote.bid_price).unwrap(),
            Rational::try_from_float_simplest(quote.ask_price).unwrap(),
        );

        let amount = (
            Rational::try_from_float_simplest(quote.bid_amount).unwrap(),
            Rational::try_from_float_simplest(quote.ask_amount).unwrap(),
        );

        CexQuote {
            exchange: quote.exchange,
            timestamp: quote.timestamp,
            price,
            token0: pair.0,
            amount,
        }
    }
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
            CexExchange::VWAP => "exchange = ''",
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
            CexExchange::VWAP => unreachable!("Cannot get fees for VWAP"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_cex_quote() {
        let pair = Pair(
            DAI_ADDRESS,
            Address::from_str("0x9f8F72aA9304c8B593d555F12eF6589cC3A579A2").unwrap(),
        );

        assert_eq!(pair.ordered(), pair);
    }
}
