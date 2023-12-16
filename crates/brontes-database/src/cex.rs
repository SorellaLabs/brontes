use std::{collections::HashMap, hash::Hash, ops::MulAssign, str::FromStr};

use alloy_primitives::Address;
use malachite::{
    num::arithmetic::traits::{Floor, ReciprocalAssign},
    Rational,
};

use crate::{DBTokenPricesDB, Pair, Quote};

#[derive(Debug, Clone)]
pub struct CexPriceMap(HashMap<Pair, Vec<CexQuote>>);

/// There should be 1 entry for how the pair is stored on the CEX and the other
/// order should be the reverse of that

impl CexPriceMap {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn wrap(map: HashMap<Pair, CexQuote>) -> Self {
        Self(map.into_iter().map(|(k, v)| (k, vec![v])).collect())
    }

    /// Assumes binance quote, for retro compatibility
    pub fn get_quote(&self, pair: &Pair) -> Option<&CexQuote> {
        self.0.get(pair).and_then(|quotes| quotes.first())
    }

    pub fn get_binance_quote(&self, pair: &Pair) -> Option<&CexQuote> {
        self.0.get(pair).and_then(|quotes| quotes.first())
    }

    pub fn get_avg_quote(&self, pair: &Pair) -> Option<CexQuote> {
        self.0.get(pair).and_then(|quotes| {
            if quotes.is_empty() {
                None
            } else {
                let sum_price = quotes
                    .iter()
                    .fold((Rational::default(), Rational::default()), |acc, q| {
                        (acc.0 + q.price.0.clone(), acc.1 + q.price.1.clone())
                    });
                let count = Rational::from(quotes.len());
                Some(CexQuote {
                    exchange:  None,
                    timestamp: quotes.last().unwrap().timestamp,
                    price:     (sum_price.0 / count.clone(), sum_price.1 / count),
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
}

impl Quote for CexQuote {
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
            let pair = Pair(
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
                    }
                })
                .collect();

            map.insert(pair, quotes);
        }

        CexPriceMap(map)
    }
}
