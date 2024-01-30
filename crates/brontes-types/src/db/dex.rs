use std::collections::HashMap;

use malachite::{num::basic::traits::One, Rational};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::pair::Pair;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DexPrices {
    pub pre_state:  Rational,
    pub post_state: Rational,
}

impl DexPrices {
    pub fn get_price(&self, post: bool) -> &Rational {
        if post {
            &self.post_state
        } else {
            &self.pre_state
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DexQuotes(pub Vec<Option<HashMap<Pair, DexPrices>>>);

impl DexQuotes {
    /// checks for price at the given tx index. if it isn't found, will look for
    /// the price at all previous indexes in the block
    pub fn price_at_or_before(&self, pair: Pair, mut tx: usize) -> Option<DexPrices> {
        if pair.0 == pair.1 {
            return Some(DexPrices { pre_state: Rational::ONE, post_state: Rational::ONE })
        }

        loop {
            if let Some(price) = self.get_price(pair, tx) {
                return Some(price.clone())
            }
            if tx == 0 {
                break
            }

            tx -= 1;
        }
        error!(?pair, "no price for pair");
        None
    }

    pub fn get_price(&self, pair: Pair, tx: usize) -> Option<&DexPrices> {
        self.0.get(tx)?.as_ref()?.get(&pair)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DexQuote(pub HashMap<Pair, DexPrices>);

impl From<DexQuoteWithIndex> for DexQuote {
    fn from(value: DexQuoteWithIndex) -> Self {
        Self(value.quote.into_iter().collect())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DexQuoteWithIndex {
    pub tx_idx: u16,
    pub quote:  Vec<(Pair, DexPrices)>,
}

impl From<DexQuote> for Vec<(Pair, DexPrices)> {
    fn from(val: DexQuote) -> Self {
        val.0.into_iter().collect()
    }
}
