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
use std::{cmp::min, default::Default, fmt, mem, ops::MulAssign};

use ahash::HashSetExt;
use alloy_primitives::{Address, TxHash};
use clickhouse::Row;
use colored::*;
use itertools::Itertools;
use malachite::{
    num::{
        basic::traits::{One, Two, Zero},
        logic::traits::SignificantBits,
    },
    Natural, Rational,
};
use redefined::{Redefined, RedefinedConvert, self_convert_redefined};
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
#[allow(unused_imports)]
use serde::{ser::SerializeSeq, Deserialize, Serialize};
use tracing::error;

use super::types::CexQuote;
use crate::{
    db::{
        cex::{quotes::CexQuoteRedefined, trades::Direction, CexExchange, RawCexQuotes},
        redefined_types::malachite::RationalRedefined,
    },
    implement_table_value_codecs_with_zc,
    normalized_actions::NormalizedSwap,
    pair::{Pair, PairRedefined},
    utils::ToFloatNearest,
    FastHashMap, FastHashSet,
};
use crate::constants::*;

const MAX_TIME_DIFFERENCE: u64 = 250_000;

pub enum CommodityClass {
    Spot,
    Futures,
    Options,
    Derivative
}

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
pub struct CexPriceMap {
    pub quotes:         FastHashMap<CexExchange, FastHashMap<Pair, Vec<CexQuote>>>,
    pub most_liquid_ex: FastHashMap<Pair, CexExchange>,
}

#[derive(
    Debug, PartialEq, Clone, serde::Serialize, rSerialize, rDeserialize, Archive, Redefined,
)]
#[redefined(CexPriceMap)]
#[redefined_attr(
    to_source = "CexPriceMap {
        quotes: self.map.into_iter().collect::<FastHashMap<_,_>>().to_source(),
        most_liquid_ex: self.most_liquid_ex.into_iter().collect::<FastHashMap<_,_>>().to_source(),
    }",
    from_source = "CexPriceMapRedefined::new(src.quotes, src.most_liquid_ex)"
)]
pub struct CexPriceMapRedefined {
    pub map:            Vec<(CexExchange, FastHashMap<PairRedefined, Vec<CexQuoteRedefined>>)>,
    pub most_liquid_ex: Vec<(PairRedefined, CexExchange)>,
}

impl CexPriceMapRedefined {
    fn new(
        map: FastHashMap<CexExchange, FastHashMap<Pair, Vec<CexQuote>>>,
        most_liquid_ex: FastHashMap<Pair, CexExchange>,
    ) -> Self {
        Self {
            map:            map
                .into_iter()
                .map(|(exch, inner_map)| (exch, FastHashMap::from_source(inner_map)))
                .collect::<Vec<_>>(),
            most_liquid_ex: most_liquid_ex
                .into_iter()
                .map(|(pair, ex)| (PairRedefined::from_source(pair), ex))
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
        Self { quotes: FastHashMap::default(), most_liquid_ex: FastHashMap::default() }
    }

    /// Retrieves the quote closest to the specified timestamp for the given
    /// pair on the exchange with the highest trading volume in that month.
    pub fn get_quote_from_most_liquid_exchange(
        &self,
        pair: &Pair,
        timestamp: u64,
        max_time_diff: Option<u64>,
    ) -> Option<FeeAdjustedQuote> {
        self.most_liquid_ex
            .get(pair)
            .or_else(|| self.most_liquid_ex.get(&pair.flip()))
            .and_then(|exchange| self.get_quote_at(pair, exchange, timestamp, max_time_diff))
    }

    pub fn get_quote_at(
        &self,
        pair: &Pair,
        exchange: &CexExchange,
        timestamp: u64,
        max_time_diff: Option<u64>,
    ) -> Option<FeeAdjustedQuote> {
        self.get_exchange_quote_at_direct(pair, exchange, timestamp, max_time_diff)
            .or_else(|| {
                self.get_exchange_quote_at_via_intermediary(
                    pair,
                    exchange,
                    timestamp,
                    max_time_diff,
                )
            })
    }

    pub fn get_exchange_quote_at_direct(
        &self,
        pair: &Pair,
        exchange: &CexExchange,
        timestamp: u64,
        max_time_diff: Option<u64>,
    ) -> Option<FeeAdjustedQuote> {
        if pair.0 == pair.1 {
            return Some(FeeAdjustedQuote::default_one_to_one());
        }

        self.quotes
            .get(exchange)
            .and_then(|quotes| {
                if let Some(exchange_quotes) = quotes.get(pair) {
                    Some((exchange_quotes, Direction::Sell))
                } else {
                    let flipped_pair = pair.flip();
                    quotes
                        .get(&flipped_pair)
                        .map(|quotes| (quotes, Direction::Buy))
                }
            })
            .and_then(|(adjusted_quotes, direction)| {
                if adjusted_quotes.is_empty() {
                    return None;
                }

                let index = adjusted_quotes.partition_point(|q| q.timestamp <= timestamp);

                let closest_quote = adjusted_quotes
                    .get(index.saturating_sub(1))
                    .into_iter()
                    .chain(adjusted_quotes.get(index))
                    .min_by_key(|&quote| (quote.timestamp as i64 - timestamp as i64).abs())?;

                let time_diff = (closest_quote.timestamp as i64 - timestamp as i64).unsigned_abs();
                let max_allowed_diff = max_time_diff.unwrap_or(MAX_TIME_DIFFERENCE);

                if time_diff > max_allowed_diff {
                    return None;
                }

                let adjusted_quote = closest_quote.adjust_for_direction(direction);

                let fees = exchange.fees(pair, &CommodityClass::Spot);

                let fee_adjusted_maker = (
                    &adjusted_quote.price.0 * (Rational::ONE - &fees.0),
                    &adjusted_quote.price.1 * (Rational::ONE - &fees.0),
                );

                let fee_adjusted_taker = (
                    &adjusted_quote.price.0 * (Rational::ONE - &fees.1),
                    &adjusted_quote.price.1 * (Rational::ONE - &fees.1),
                );

                Some(FeeAdjustedQuote {
                    exchange:    *exchange,
                    timestamp:   adjusted_quote.timestamp,
                    pairs:       vec![*pair],
                    price_maker: (fee_adjusted_maker.0, fee_adjusted_maker.1),
                    price_taker: (fee_adjusted_taker.0, fee_adjusted_taker.1),
                    amount:      adjusted_quote.amount,
                })
            })
    }

    pub fn get_exchange_quote_at_via_intermediary(
        &self,
        pair: &Pair,
        exchange: &CexExchange,
        timestamp: u64,
        max_time_diff: Option<u64>,
    ) -> Option<FeeAdjustedQuote> {
        let intermediaries = self.calculate_intermediary_addresses(exchange, pair);

        intermediaries
            .iter()
            .filter_map(|&intermediary| {
                let pair0 = Pair(pair.0, intermediary);
                let pair1 = Pair(intermediary, pair.1);

                if let (Some(quote1), Some(quote2)) = (
                    self.get_exchange_quote_at_direct(&pair0, exchange, timestamp, max_time_diff),
                    self.get_exchange_quote_at_direct(&pair1, exchange, timestamp, max_time_diff),
                ) {
                    let combined_price_maker = (
                        &quote1.price_maker.0 * &quote2.price_maker.0,
                        &quote1.price_maker.1 * &quote2.price_maker.1,
                    );

                    let combined_price_taker = (
                        &quote1.price_taker.0 * &quote2.price_taker.0,
                        &quote1.price_taker.1 * &quote2.price_taker.1,
                    );

                    if quote2.price_maker.0 == Rational::ZERO {
                        return None;
                    }

                    let normalized_bbo_amount: (Rational, Rational) = (
                        min(quote1.amount.0.clone(), &quote2.amount.0 / &quote2.price_maker.0),
                        min(&quote1.amount.1 * quote1.price_maker.1, quote2.amount.1.clone()),
                    );

                    Some(FeeAdjustedQuote {
                        exchange:    *exchange,
                        pairs:       vec![pair0, pair1],
                        timestamp:   std::cmp::max(quote1.timestamp, quote2.timestamp),
                        price_maker: combined_price_maker,
                        price_taker: combined_price_taker,
                        amount:      normalized_bbo_amount,
                    })
                } else {
                    None
                }
            })
            .max_by(|a, b| a.amount.0.cmp(&b.amount.0))
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
    pub fn get_vm_quote(&self, pair: &Pair, exchange: &CexExchange) -> Option<FeeAdjustedQuote> {
        if pair.0 == pair.1 {
            return Some(FeeAdjustedQuote::default_one_to_one());
        }

        self.quotes
            .get(exchange)
            .and_then(|quotes| {
                if let Some(exchange_quotes) = quotes.get(pair) {
                    Some(
                        exchange_quotes
                            .iter()
                            .map(|q| q.adjust_for_direction(Direction::Sell))
                            .collect_vec(),
                    )
                } else {
                    let flipped_pair = pair.flip();
                    quotes.get(&flipped_pair).map(|quotes| {
                        quotes
                            .iter()
                            .map(|q| q.adjust_for_direction(Direction::Buy))
                            .collect_vec()
                    })
                }
            })
            .and_then(|adjusted_quotes| {
                if adjusted_quotes.is_empty() {
                    None
                } else {
                    let mut cumulative_bbo = (Rational::ZERO, Rational::ZERO);
                    let mut volume_price = (Rational::ZERO, Rational::ZERO);

                    let timestamp = adjusted_quotes[0].timestamp;

                    for quote in adjusted_quotes {
                        cumulative_bbo.0 += &quote.amount.0;
                        cumulative_bbo.1 += &quote.amount.1;

                        volume_price.0 += &quote.price.0 * &quote.amount.0;
                        volume_price.1 += &quote.price.1 * &quote.amount.1;
                    }

                    let volume_weighted_bid = volume_price.0 / &cumulative_bbo.0;
                    let volume_weighted_ask = volume_price.1 / &cumulative_bbo.1;

                    let fees = exchange.fees(&pair, &CommodityClass::Spot);

                    let fee_adjusted_maker = (
                        &volume_weighted_bid * (Rational::ONE - &fees.0),
                        &volume_weighted_ask * (Rational::ONE - &fees.0),
                    );

                    let fee_adjusted_taker = (
                        volume_weighted_bid * (Rational::ONE - &fees.1),
                        volume_weighted_ask * (Rational::ONE - &fees.1),
                    );

                    Some(FeeAdjustedQuote {
                        exchange: *exchange,
                        timestamp,
                        pairs: vec![*pair],
                        price_maker: (fee_adjusted_maker.0, fee_adjusted_maker.1),
                        price_taker: (fee_adjusted_taker.0, fee_adjusted_taker.1),
                        // This is the sum of bid and ask amounts for each quote in this time
                        // window, exchange & pair. This does not represent the total amount
                        // available
                        amount: (cumulative_bbo.0, cumulative_bbo.1),
                    })
                }
            })
    }

    /// Retrieves a CEX quote for a given token pair using an intermediary
    /// asset.
    ///
    /// This method is used when a direct quote for the pair is not available.
    /// It attempts to construct a quote for `pair` by finding a path
    /// through a common intermediary asset as provided by the `exchange`.
    pub fn get_vm_quote_via_intermediary(
        &self,
        pair: &Pair,
        exchange: &CexExchange,
    ) -> Option<FeeAdjustedQuote> {
        let intermediaries = self.calculate_intermediary_addresses(exchange, pair);

        intermediaries
            .iter()
            .filter_map(|&intermediary| {
                let pair0 = Pair(pair.0, intermediary);
                let pair1 = Pair(intermediary, pair.1);

                if let (Some(quote1), Some(quote2)) =
                    (self.get_vm_quote(&pair0, exchange), self.get_vm_quote(&pair1, exchange))
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
                        min(quote1.amount.0.clone(), &quote2.amount.0 / &quote2.price_maker.0),
                        min(&quote1.amount.1 * quote1.price_maker.1, quote2.amount.1.clone()),
                    );

                    Some(FeeAdjustedQuote {
                        exchange:    *exchange,
                        pairs:       vec![pair0, pair1],
                        timestamp:   std::cmp::max(quote1.timestamp, quote2.timestamp),
                        price_maker: combined_price_maker,
                        price_taker: combined_price_taker,
                        amount:      normalized_bbo_amount,
                    })
                } else {
                    None
                }
            })
            .max_by(|a, b| a.amount.0.cmp(&b.amount.0))
    }

    /// Retrieves a CEX quote for a given token pair directly or via an
    /// intermediary
    pub fn get_vm_quote_direct_or_via_intermediary(
        &self,
        pair: &Pair,
        exchange: &CexExchange,
    ) -> Option<FeeAdjustedQuote> {
        self.get_vm_quote(pair, exchange)
            .or_else(|| self.get_vm_quote_via_intermediary(pair, exchange))
    }

    pub fn get_global_vm_quote(
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
                pairs:       exchange_quotes.first().unwrap().pairs.clone(),
                timestamp:   avg_timestamp,
                price_maker: (volume_weighted_bid_maker, volume_weighted_ask_maker),
                price_taker: (volume_weighted_bid_taker, volume_weighted_ask_taker),
                amount:      avg_amount,
            })
        }
    }

    pub fn get_binance_quote(&self, pair: &Pair) -> Option<FeeAdjustedQuote> {
        self.get_vm_quote(pair, &CexExchange::Binance)
    }

    fn calculate_intermediary_addresses(
        &self,
        exchange: &CexExchange,
        pair: &Pair,
    ) -> FastHashSet<Address> {
        let (token_a, token_b) = (pair.0, pair.1);
        let mut connected_to_a = FastHashSet::new();
        let mut connected_to_b = FastHashSet::new();

        self.quotes
            .iter()
            .filter(|(venue, _)| *venue == exchange)
            .flat_map(|(_, pairs)| pairs.keys())
            .for_each(|trade_pair| {
                if trade_pair.0 == token_a {
                    connected_to_a.insert(trade_pair.1);
                } else if trade_pair.1 == token_a {
                    connected_to_a.insert(trade_pair.0);
                }

                if trade_pair.0 == token_b {
                    connected_to_b.insert(trade_pair.1);
                } else if trade_pair.1 == token_b {
                    connected_to_b.insert(trade_pair.0);
                }
            });

        connected_to_a
            .intersection(&connected_to_b)
            .cloned()
            .collect()
    }

    pub fn quote_count(&self) -> usize {
        self.quotes
            .values()
            .flat_map(|v| v.values())
            .map(|v| v.len())
            .sum()
    }

    pub fn pair_count(&self) -> usize {
        self.quotes.values().map(|v| v.len()).sum()
    }

    pub fn total_counts(&self) -> (usize, usize) {
        (self.quote_count(), self.pair_count())
    }
}

#[allow(dead_code)]
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
    pub pairs:       Vec<Pair>,
    /// Best fee adjusted Bid & Ask price (maker)
    pub price_maker: (Rational, Rational),
    /// Best fee adjusted Bid & Ask price (taker)
    pub price_taker: (Rational, Rational),
    /// Bid & Ask amount
    pub amount:      (Rational, Rational),
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

        Ok(())
    }
}
impl FeeAdjustedQuote {
    pub fn default_one_to_one() -> Self {
        Self {
            price_maker: (Rational::ONE, Rational::ONE),
            price_taker: (Rational::ONE, Rational::ONE),
            ..Default::default()
        }
    }

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

impl From<(Pair, RawCexQuotes)> for CexQuote {
    fn from(value: (Pair, RawCexQuotes)) -> Self {
        let (_pair, quote) = value;

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
            // token0: pair.0,
            // token1: pair.1,
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

pub fn size_of_cex_price_map(price_map: &CexPriceMap) -> usize {
    let mut total_size = mem::size_of_val(price_map);

    // Size of quotes
    total_size += size_of_nested_fast_hash_map(&price_map.quotes);

    // Size of most_liquid_ex
    total_size += size_of_fast_hash_map(&price_map.most_liquid_ex);

    total_size
}

fn size_of_nested_fast_hash_map(
    map: &FastHashMap<CexExchange, FastHashMap<Pair, Vec<CexQuote>>>,
) -> usize {
    let mut size = mem::size_of_val(map);

    for (exchange, inner_map) in map {
        size += mem::size_of_val(exchange);
        size += size_of_fast_hash_map(inner_map);

        for (pair, quotes) in inner_map {
            size += mem::size_of_val(pair);
            size += mem::size_of_val(quotes);
            size += quotes.iter().map(size_of_cex_quote).sum::<usize>();
        }
    }

    size
}

fn size_of_fast_hash_map<K, V>(map: &FastHashMap<K, V>) -> usize {
    mem::size_of_val(map) + (map.capacity() * (mem::size_of::<K>() + mem::size_of::<V>()))
}

fn size_of_cex_quote(quote: &CexQuote) -> usize {
    mem::size_of_val(quote)
        + size_of_rational(&quote.price.0)
        + size_of_rational(&quote.price.1)
        + size_of_rational(&quote.amount.0)
        + size_of_rational(&quote.amount.1)
        + 8
}

fn size_of_rational(rational: &Rational) -> usize {
    let size = mem::size_of::<Rational>();
    // Add the size of heap-allocated data in Natural
    size + size_of_natural(rational.numerator_ref()) + size_of_natural(rational.denominator_ref())
}

fn size_of_natural(natural: &Natural) -> usize {
    let capacity = (natural.significant_bits() / 64 + 1) as usize;
    mem::size_of::<Natural>() + capacity * mem::size_of::<usize>()
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

impl CexExchange {
    pub fn to_clickhouse_filter(&self) -> &str {
        match self {
            CexExchange::Binance => "(c.exchange = 'binance' or c.exchange = 'binance-futures')",
            CexExchange::Bitmex => "c.exchange = 'bitmex'",
            CexExchange::Deribit => "c.exchange = 'deribit'",
            CexExchange::Okex => "(c.exchange = 'okex' or c.exchange = 'okex-swap')",
            CexExchange::Coinbase => "c.exchange = 'coinbase'",
            CexExchange::Kraken => "c.exchange = 'kraken'",
            CexExchange::BybitSpot => "(c.exchange = 'bybit-spot' or c.exchange = 'bybit')",
            CexExchange::Kucoin => "c.exchange = 'kucoin'",
            CexExchange::Upbit => "c.exchange = 'upbit'",
            CexExchange::Huobi => "c.exchange = 'huobi'",
            CexExchange::GateIo => "c.exchange = 'gate-io;",
            CexExchange::Bitstamp => "c.exchange = 'bitstamp'",
            CexExchange::Gemini => "c.exchange = 'gemini'",
            CexExchange::Unknown => "c.exchange = ''",
            CexExchange::Average => "c.exchange = ''",
            CexExchange::VWAP => "c.exchange = ''",
            CexExchange::OptimisticVWAP => "c.exchange = ''",
        }
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
    /// Does not account for special market maker rebate programs or special
    /// pairs
    ///
    /// TODO: Account for special fee pairs & stableswap rates
    /// TODO: Account for futures & spot fee deltas
    pub fn fees(&self, pair: &Pair, trade_type: &CommodityClass) -> (Rational, Rational) {
        let (maker, taker) = match self {
            CexExchange::Binance => {
                match trade_type {
                    CommodityClass::Spot =>
                        if Self::BINANCE_SPOT_PROMO_FEE_TYPE1_PAIRS.iter().any(|p| p.eq_ordered(pair)) {
                            ("0.0", "0.0") // https://www.binance.com/en/fee/tradingPromote
                        } else if Self::BINANCE_SPOT_PROMO_FEE_TYPE2_PAIRS.iter().any(|p| p.eq_ordered(pair)) {
                            ("0.0", "0.00024") // https://www.binance.com/en/fee/tradingPromote
                        } else if pair.0 == USDC_ADDRESS || pair.1 == USDC_ADDRESS {
                            ("0.00012", "0.0001425") // https://www.binance.com/en/fee/trading
                        } else {
                            ("0.00012", "0.00024") // https://www.binance.com/en/fee/trading
                        },
                    CommodityClass::Derivative => ("0.0003", "0.0003"), // https://www.binance.com/en/fee/optionsTrading
                    CommodityClass::Futures | CommodityClass::Options => todo!()
                }
            },
            CexExchange::Bitmex =>
                match trade_type {
                    CommodityClass::Spot => ("0.001", "0.001"), // https://www.bitmex.com/wallet/fees/spot
                    CommodityClass::Derivative => ("-0.000125", "0.000175"), // https://www.bitmex.com/wallet/fees/derivatives
                    CommodityClass::Futures | CommodityClass::Options => todo!()
                }
            CexExchange::Deribit =>
                match trade_type {
                    CommodityClass::Spot => ("0.0", "0.0"), // https://www.deribit.com/kb/fees
                    CommodityClass::Derivative => ("-0.0001", "0.0005"), // https://www.deribit.com/kb/fees
                    CommodityClass::Futures | CommodityClass::Options => todo!()
                }
            CexExchange::Okex => ("-0.0001", "0.00015"), // https://tr.okx.com/fees
            CexExchange::Coinbase =>
                // https://help.coinbase.com/en/exchange/trading-and-funding/exchange-fees
                if USD_STABLES_BY_ADDRESS.iter().any(|a| pair.0 == *a || pair.1 == *a) ||
                    WBTC_ADDRESS == pair.0 || WBTC_ADDRESS == pair.1 {
                    ("0.0", "0.00001")
                } else {
                    ("0", "0.0005")
                },
            CexExchange::Kraken =>
                match trade_type {
                    CommodityClass::Spot => ("0.0", "0.001"), // https://www.kraken.com/features/fee-schedule#spot-crypto
                    CommodityClass::Derivative =>  ("0.0", "0.0001"), // https://www.kraken.com/features/fee-schedule#futures
                    CommodityClass::Futures | CommodityClass::Options => todo!()
                },
            CexExchange::BybitSpot =>
                // https://www.bybit.com/en/help-center/article/Trading-Fee-Structure
                match trade_type {
                    CommodityClass::Spot => ("0.00005", "0.00015"),
                    CommodityClass::Derivative => if USDC_ADDRESS == pair.0 || USDC_ADDRESS == pair.1 {
                        ("0.0", "0.0001")
                    } else {
                        ("0.0", "0.00025")
                    }
                    CommodityClass::Futures | CommodityClass::Options => todo!()
                }
            CexExchange::Kucoin => 
                // https://www.kucoin.com/vip/privilege
                match trade_type {
                    CommodityClass::Spot =>
                        if Self::KUCOIN_CLASS_C_BASE_COINS.iter().any(|a| pair.0 == *a) {
                            ("-0.00005", "0.00075")
                        } else if Self::KUCOIN_CLASS_B_BASE_COINS.iter().any(|a| pair.0 == *a) {
                            ("-0.00005", "0.0005")
                        } else if Self::KUCOIN_CLASS_A_BASE_COINS.iter().any(|a| pair.0 == *a) {
                            ("-0.00005", "0.00025")
                        } else if Self::KUCOIN_TOP_BASE_COINS.iter().any(|a| pair.0 == *a) {
                            ("-0.00005", "0.00025")
                        } else {
                            ("-0.00005", "0.00025")
                        },
                    CommodityClass::Derivative => ("-0.00008", "0.00025"),
                    CommodityClass::Futures | CommodityClass::Options => todo!()
                },
            CexExchange::Upbit => ("0.0002", "0.0002"), // https://sg.upbit.com/service_center/guide
            CexExchange::Huobi => 
                match trade_type {
                    CommodityClass::Spot => ("0.000097", "0.000193"), // https://www.htx.com/zh-cn/support/360000312282
                    CommodityClass::Derivative => ("-0.00005", "0.0002"), // https://www.htx.com/zh-cn/support/360000113122
                    CommodityClass::Futures | CommodityClass::Options => todo!()
                }
            CexExchange::GateIo => ("0.0", "0.0002"), // https://www.gate.io/fee (curl, search for spot_feelist)
            CexExchange::Bitstamp => ("0", "0.0003"), // https://www.bitstamp.net/fee-schedule/
            CexExchange::Gemini => ("0", "0.0003"), // https://www.gemini.com/fees/api-fee-schedule#section-gemini-stablecoin-fee-schedule
            CexExchange::Average => {
                unreachable!("Cannot get fees for cross exchange average quote")
            }
            CexExchange::Unknown => unreachable!("Unknown cex exchange"),
            CexExchange::VWAP | CexExchange::OptimisticVWAP => {
                unreachable!("Cannot get fees for VWAP")
            }
        };
        (Rational::from_sci_string_simplest(maker).unwrap(), Rational::from_sci_string_simplest(taker).unwrap())
    }

    // https://www.binance.com/en/fee/tradingPromote
    const BINANCE_SPOT_PROMO_FEE_TYPE1_PAIRS: [Pair; 8] = [
        Pair(WBTC_ADDRESS, FDUSD_ADDRESS),
        Pair(FDUSD_ADDRESS, USDT_ADDRESS),
        Pair(ETH_ADDRESS, FDUSD_ADDRESS),
        Pair(LINK_ADDRESS, FDUSD_ADDRESS),
        Pair(AEUR_ADDRESS, USDT_ADDRESS),
        Pair(TUSD_ADDRESS, USDT_ADDRESS),
        Pair(USDC_ADDRESS, USDT_ADDRESS),
        Pair(USDP_ADDRESS, USDT_ADDRESS),
    ];

    // https://www.binance.com/en/fee/tradingPromote
    const BINANCE_SPOT_PROMO_FEE_TYPE2_PAIRS: [Pair; 50] = [
        Pair(ACE_ADDRESS, FDUSD_ADDRESS),
        Pair(ADA_ADDRESS, FDUSD_ADDRESS),
        Pair(AEVO_ADDRESS, FDUSD_ADDRESS),
        Pair(AI_ADDRESS, FDUSD_ADDRESS),
        Pair(ALT_ADDRESS, FDUSD_ADDRESS),
        Pair(ARB_ADDRESS, FDUSD_ADDRESS),
        Pair(ARKM_ADDRESS, FDUSD_ADDRESS),
        Pair(AUCTION_ADDRESS, FDUSD_ADDRESS),
        Pair(AXL_ADDRESS, FDUSD_ADDRESS),
        Pair(BLZ_ADDRESS, FDUSD_ADDRESS),
        Pair(CHZ_ADDRESS, FDUSD_ADDRESS),
        Pair(CYBER_ADDRESS, FDUSD_ADDRESS),
        Pair(DYDX_ADDRESS, FDUSD_ADDRESS),
        Pair(ENA_ADDRESS, FDUSD_ADDRESS),
        Pair(ENS_ADDRESS, FDUSD_ADDRESS),
        Pair(ETHFI_ADDRESS, FDUSD_ADDRESS),
        Pair(FET_ADDRESS, FDUSD_ADDRESS),
        Pair(FLOKI_ADDRESS, FDUSD_ADDRESS),
        Pair(FTM_ADDRESS, FDUSD_ADDRESS),
        Pair(GALA_ADDRESS, FDUSD_ADDRESS),
        Pair(GRT_ADDRESS, FDUSD_ADDRESS),
        Pair(INJ_ADDRESS, FDUSD_ADDRESS),
        Pair(JUP_ADDRESS, FDUSD_ADDRESS),
        Pair(LDO_ADDRESS, FDUSD_ADDRESS),
        Pair(MATIC_ADDRESS, FDUSD_ADDRESS),
        Pair(MEME_ADDRESS, FDUSD_ADDRESS),
        Pair(OMNI_ADDRESS, FDUSD_ADDRESS),
        Pair(PENDLE_ADDRESS, FDUSD_ADDRESS),
        Pair(PEOPLE_ADDRESS, FDUSD_ADDRESS),
        Pair(PEPE_ADDRESS, FDUSD_ADDRESS),
        Pair(PIXEL_ADDRESS, FDUSD_ADDRESS),
        Pair(PORTAL_ADDRESS, FDUSD_ADDRESS),
        Pair(REZ_ADDRESS, FDUSD_ADDRESS),
        Pair(RNDR_ADDRESS, FDUSD_ADDRESS),
        Pair(SAND_ADDRESS, FDUSD_ADDRESS),
        Pair(SHIB_ADDRESS, FDUSD_ADDRESS),
        Pair(STRK_ADDRESS, FDUSD_ADDRESS),
        Pair(SUPER_ADDRESS, FDUSD_ADDRESS),
        Pair(UNI_ADDRESS, FDUSD_ADDRESS),
        Pair(W_ADDRESS, FDUSD_ADDRESS),
        Pair(WLD_ADDRESS, FDUSD_ADDRESS),
        Pair(ADA_ADDRESS, TUSD_ADDRESS),
        Pair(ARB_ADDRESS, TUSD_ADDRESS),
        Pair(ARKM_ADDRESS, TUSD_ADDRESS),
        Pair(WBTC_ADDRESS, TUSD_ADDRESS),
        Pair(CYBER_ADDRESS, TUSD_ADDRESS),
        Pair(ETH_ADDRESS, TUSD_ADDRESS),
        Pair(MATIC_ADDRESS, TUSD_ADDRESS),
        Pair(MAV_ADDRESS, TUSD_ADDRESS),
        Pair(PEPE_ADDRESS, TUSD_ADDRESS),
    ];

    const KUCOIN_TOP_BASE_COINS: [Address; 12] = [
        WBTC_ADDRESS,
        ETH_ADDRESS,
        // XRP_ADDRESS,
        // SOL_ADDRESS,
        ADA_ADDRESS,
        // DOGE_ADDRESS,
        // TRX_ADDRESS,
        // AVAX_ADDRESS,
        MATIC_ADDRESS,
        LINK_ADDRESS,
        // DOT_ADDRESS,
        // LTC_ADDRESS,
        SHIB_ADDRESS,
        // BCH_ADDRESS,
        // ATOM_ADDRESS,
        UNI_ADDRESS,
        // XMR_ADDRESS,
        // ETC_ADDRESS,
        // LUNC_ADDRESS,
        // TON_ADDRESS,
        DAI_ADDRESS,
        KCS_ADDRESS,
        USDT_ADDRESS,
        USDC_ADDRESS,
        USDP_ADDRESS,
    ];

    const KUCOIN_CLASS_A_BASE_COINS: [Address; 39] = [
        AEVO_ADDRESS, ARB_ADDRESS, ARKM_ADDRESS, AUCTION_ADDRESS, BLZ_ADDRESS, BNB_ADDRESS, CHZ_ADDRESS, CYBER_ADDRESS,
        DYDX_ADDRESS, ENA_ADDRESS, ENS_ADDRESS, ETHFI_ADDRESS, FET_ADDRESS, FLOKI_ADDRESS, FTM_ADDRESS, GRT_ADDRESS,
        INJ_ADDRESS, JUP_ADDRESS, LDO_ADDRESS, MAV_ADDRESS, MEME_ADDRESS, OMNI_ADDRESS, PAXG_ADDRESS, PENDLE_ADDRESS,
        PEOPLE_ADDRESS, PEPE_ADDRESS, PIXEL_ADDRESS, PORTAL_ADDRESS, PYUSD_ADDRESS, REZ_ADDRESS, SAND_ADDRESS, STRK_ADDRESS,
        SUPER_ADDRESS, TUSD_ADDRESS, USDD_ADDRESS, USTC_ADDRESS, W_ADDRESS, WBTC_ADDRESS, WLD_ADDRESS,
    ];

    const KUCOIN_CLASS_B_BASE_COINS: [Address; 5] = [
        ACE_ADDRESS, AI_ADDRESS, ALT_ADDRESS, RNDR_ADDRESS, USDE_ADDRESS,
    ];

    const KUCOIN_CLASS_C_BASE_COINS: [Address; 0] = [];
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
