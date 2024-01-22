use std::collections::HashMap;

use malachite::Rational;
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::extra_processing::Pair;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DexQuotes(pub Vec<Option<HashMap<Pair, Rational>>>);

impl DexQuotes {
    /// checks for price at the given tx index. if it isn't found, will look for
    /// the price at all previous indexes in the block
    pub fn price_at_or_before(&self, pair: Pair, mut tx: usize) -> Option<Rational> {
        if pair.0 == pair.1 {
            return Some(Rational::from(1))
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

    pub fn get_price(&self, pair: Pair, tx: usize) -> Option<&Rational> {
        self.0.get(tx)?.as_ref()?.get(&pair)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DexQuote(pub HashMap<Pair, Rational>);

impl From<DexQuoteWithIndex> for DexQuote {
    fn from(value: DexQuoteWithIndex) -> Self {
        Self(value.quote.into_iter().collect())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DexQuoteWithIndex {
    pub tx_idx: u16,
    pub quote:  Vec<(Pair, Rational)>,
}

impl From<DexQuote> for Vec<(Pair, Rational)> {
    fn from(val: DexQuote) -> Self {
        val.0
            .into_iter()
            //.map(|(x, y)| (Redefined_Pair::from_source(x), Redefined_Rational::from_source(y)))
            .collect()
    }
}
