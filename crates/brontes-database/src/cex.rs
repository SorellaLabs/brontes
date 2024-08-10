use std::{collections::HashMap, hash::Hash, ops::MulAssign, str::FromStr};

use alloy_primitives::Address;
use brontes_types::extra_processing::Pair;
use malachite::{
    num::arithmetic::traits::{Floor, Reciprocal, ReciprocalAssign},
    Rational,
};

use crate::{DBTokenPricesDB, Quote};

/// Each pair is entered into the map with the addresses in order by value:
/// Ergo if token0 < token1, then the pair is (token0, token1)
/// So when we query the map we order the addresses in the pair and then query
/// the quote provides us with the actual token0 so we can interpret the price
/// in any direction
#[derive(Debug, Clone)]
pub struct CexPriceMap(HashMap<Pair, Vec<CexQuote>>);

impl CexPriceMap {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn wrap(map: HashMap<Pair, CexQuote>) -> Self {
        Self(map.into_iter().map(|(k, v)| (k, vec![v])).collect())
    }

    /// Assumes binance quote, for retro compatibility
    pub fn get_quote(&self, pair: &Pair) -> Option<CexQuote> {
        let ordered_pair = pair.ordered();
        self.0.get(&ordered_pair).and_then(|quotes| {
            quotes.first().map(|quote| {
                if quote.token0 == pair.0 {
                    quote.clone()
                } else {
                    let mut reciprocal_quote = quote.clone();
                    reciprocal_quote.inverse_price(); // Modify the price to its reciprocal
                    reciprocal_quote
                }
            })
        })
    }

    pub fn get_binance_quote(&self, pair: &Pair) -> Option<CexQuote> {
        let ordered_pair = pair.ordered();
        self.0.get(&ordered_pair).and_then(|quotes| {
            quotes.first().map(|quote| {
                if quote.token0 == pair.0 {
                    quote.clone()
                } else {
                    let mut reciprocal_quote = quote.clone();
                    reciprocal_quote.inverse_price(); // Modify the price to its reciprocal
                    reciprocal_quote
                }
            })
        })
    }

    pub fn get_avg_quote(&self, pair: &Pair) -> Option<CexQuote> {
        let ordered_pair = pair.ordered();
        self.0.get(&ordered_pair).and_then(|quotes| {
            if quotes.is_empty() {
                None
            } else {
                let (sum_price, count) = quotes.iter().fold(
                    ((Rational::default(), Rational::default()), 0),
                    |(acc, cnt), q| {
                        let mut quote = q.clone();
                        if quote.token0 != pair.0 {
                            quote.inverse_price();
                        }
                        ((acc.0 + quote.price.0, acc.1 + quote.price.1), cnt + 1)
                    },
                );
                let count = Rational::from(count);
                Some(CexQuote {
                    exchange:  None,
                    timestamp: quotes.last().unwrap().timestamp,
                    price:     (sum_price.0 / count.clone(), sum_price.1 / count),
                    token0:    pair.0,
                })
            }
        })
    }
}

#[derive(Debug, Clone, Hash, Eq, Default)]
pub struct CexQuote {
    pub exchange:  Option<String>,
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

impl From<Vec<DBTokenPricesDB>> for CexPriceMap {
    fn from(value: Vec<DBTokenPricesDB>) -> Self {
        let mut map: HashMap<Pair, Vec<CexQuote>> = HashMap::new();

        for token_info in value {
            let pair = Pair::map_key(
                Address::from_str(&token_info.key.0).unwrap(),
                Address::from_str(&token_info.key.1).unwrap(),
            );

            let quotes: Vec<CexQuote> = token_info
                .val
                .into_iter()
                .map(|exchange_price| {
                    CexQuote {
                        exchange:  Some(exchange_price.exchange),
                        timestamp: exchange_price.val.0,
                        price:     (
                            Rational::try_from(exchange_price.val.1).unwrap(), /* Conversion to
                                                                                * Rational */
                            Rational::try_from(exchange_price.val.2).unwrap(),
                        ),
                        token0:    Address::from_str(&token_info.key.0).unwrap(),
                    }
                })
                .collect();

            map.insert(pair, quotes);
        }

        CexPriceMap(map)
    }
}
