use std::{
    fmt,
    fmt::{Display, Formatter},
    ops::MulAssign,
};

use malachite::{num::arithmetic::traits::Reciprocal, Rational};
use redefined::Redefined;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::Serialize;
use crate::{
    db::{
        cex::{quotes::download::RawCexQuotes, trades::Direction, CexExchange},
        redefined_types::malachite::RationalRedefined
    },
    utils::ToFloatNearest,
};
use clickhouse::Row;

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
    pub exchange:       CexExchange,
    pub timestamp:      u64,
    /// Best Bid & Ask price
    pub price:          (Rational, Rational),
    /// Bid & Ask amount
    pub amount:         (Rational, Rational),
    // pub token0:         Address,
    // pub token1:         Address,
}

impl Display for CexQuote {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Exchange: {}\nTimestamp: {}\nBest Ask Price: {:.2}\nBest Bid Price: {:.2}\n",
            self.exchange,
            self.timestamp,
            self.price.0.clone().to_float(),
            self.price.1.clone().to_float(),
        )
    }
}

impl From<RawCexQuotes> for CexQuote {
    fn from(value: RawCexQuotes) -> Self {
        let price = (
            Rational::try_from_float_simplest(value.bid_price).unwrap(),
            Rational::try_from_float_simplest(value.ask_price).unwrap(),
        );

        let amount = (
            Rational::try_from_float_simplest(value.bid_amount).unwrap(),
            Rational::try_from_float_simplest(value.ask_amount).unwrap(),
        );

        CexQuote { exchange: value.exchange, timestamp: value.timestamp, price, amount }
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

    pub fn adjust_for_direction(&self, direction: Direction) -> Self {
        match direction {
            Direction::Buy => Self {
                exchange:  self.exchange,
                timestamp: self.timestamp,
                price:     self.price.clone(),
                amount:    (&self.amount.0 * &self.price.0, &self.amount.1 * &self.price.1),
            },
            Direction::Sell => Self {
                exchange:  self.exchange,
                timestamp: self.timestamp,
                price:     (self.price.0.clone().reciprocal(), self.price.1.clone().reciprocal()),
                amount:    self.amount.clone(),
            },
        }
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
