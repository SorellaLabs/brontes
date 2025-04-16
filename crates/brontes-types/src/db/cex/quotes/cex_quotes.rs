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
use redefined::{Redefined, RedefinedConvert};
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
#[allow(unused_imports)]
use serde::{ser::SerializeSeq, Deserialize, Serialize};
use tracing::warn;

use super::types::CexQuote;
use crate::{
    db::{
        cex::{exchanges, quotes::CexQuoteRedefined, trades::Direction, CexExchange},
        redefined_types::malachite::RationalRedefined,
    },
    implement_table_value_codecs_with_zc,
    normalized_actions::NormalizedSwap,
    pair::{Pair, PairRedefined},
    utils::ToFloatNearest,
    FastHashMap, FastHashSet,
};

#[derive(Debug, Clone, Row, PartialEq, Eq)]
pub struct CexPriceMap {
    pub quotes:         FastHashMap<CexExchange, FastHashMap<Pair, Vec<CexQuote>>>,
    pub most_liquid_ex: FastHashMap<Pair, Vec<CexExchange>>,
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
    pub most_liquid_ex: Vec<(PairRedefined, Vec<CexExchange>)>,
}

impl CexPriceMapRedefined {
    fn new(
        map: FastHashMap<CexExchange, FastHashMap<Pair, Vec<CexQuote>>>,
        most_liquid_ex: FastHashMap<Pair, Vec<CexExchange>>,
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
            .and_then(|exchanges| {
                for exchange in exchanges {
                    tracing::debug!(?exchange, ?pair);
                    let res = self.get_quote_at(pair, exchange, timestamp, max_time_diff);
                    if res.is_some() {
                        return res
                    }
                }
                None
            })
            .or_else(|| {
                tracing::debug!(
                    ?pair,
                    "no most liquid exchange found for pair, trying binance via intermediary"
                );
                let exchanges = vec![CexExchange::Binance, CexExchange::Coinbase];

                for exchange in exchanges {
                    if let Some(quote) = self.get_exchange_quote_at_via_intermediary(
                        pair,
                        &exchange,
                        timestamp,
                        max_time_diff,
                    ) {
                        return Some(quote);
                    }

                    if let Some(quote) = self.get_exchange_quote_at_via_intermediary(
                        &pair.flip(),
                        &exchange,
                        timestamp,
                        max_time_diff,
                    ) {
                        return Some(quote);
                    }
                }
                None
            })
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
        _max_time_diff: Option<u64>,
    ) -> Option<FeeAdjustedQuote> {
        if pair.0 == pair.1 {
            return Some(FeeAdjustedQuote::default_one_to_one())
        }

        tracing::trace!(target: "cex_quotes::lookup", ?pair, ?exchange, %timestamp, "Attempting direct lookup");

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
                    tracing::debug!(?pair, ?exchange, "no quotes");
                    return None
                }

                let index = adjusted_quotes.partition_point(|q| q.timestamp <= timestamp);

                let closest_quote = adjusted_quotes.get(index.saturating_sub(1));

                if closest_quote.is_none() {
                    tracing::debug!(target: "cex_quotes::lookup", ?pair, ?exchange, %timestamp, index, "Direct lookup: Found quotes, but none at or before the target timestamp");
                    return None;
                }

                let adjusted_quote = closest_quote.unwrap().adjust_for_direction(direction);

                let fees = exchange.fees();

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
                        return None
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
            return Some(FeeAdjustedQuote::default_one_to_one())
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

                    let fees = exchange.fees();

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
    warn!(
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
    warn!(
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
