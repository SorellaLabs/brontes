use clickhouse::Row;
use itertools::Itertools;
use malachite::{
    num::arithmetic::traits::{Reciprocal, ReciprocalAssign},
    Rational,
};
use redefined::{Redefined, RedefinedConvert};
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};

use super::{raw_cex_trades::RawCexTrades, time_window_vwam::Direction};
use crate::{
    db::{cex::CexExchange, redefined_types::malachite::RationalRedefined},
    implement_table_value_codecs_with_zc,
    pair::{Pair, PairRedefined},
    FastHashMap,
};
type RedefinedTradeMapVec = Vec<(PairRedefined, Vec<CexTradesRedefined>)>;

#[derive(Debug, Default, Clone, Row, PartialEq, Eq, Serialize)]
pub struct CexTradeMap(pub FastHashMap<CexExchange, FastHashMap<Pair, Vec<CexTrades>>>);

impl CexTradeMap {
    pub fn from_redefined(map: Vec<(CexExchange, RedefinedTradeMapVec)>) -> Self {
        Self(
            map.into_iter()
                .map(|(ex, trades)| {
                    (
                        ex,
                        trades.into_iter().fold(
                            FastHashMap::default(),
                            |mut acc: FastHashMap<Pair, Vec<CexTrades>>, (pair, trades)| {
                                let trades = trades
                                    .into_iter()
                                    .map(|t| t.to_source())
                                    // ensure all trades sorted by timestamp
                                    .sorted_unstable_by_key(|k| k.timestamp);

                                acc.entry(pair.to_source()).or_default().extend(trades);
                                acc
                            },
                        ),
                    )
                })
                .collect(),
        )
    }

    /// merges in another map extending each of the pairs trades to the current
    /// map
    pub fn merge_in_map(
        &mut self,
        other: Self,
    ) -> FastHashMap<CexExchange, FastHashMap<Pair, usize>> {
        // generate offset list for proper removal of each pair
        other
            .0
            .into_iter()
            .fold(FastHashMap::default(), |mut acc, (exchange, pairs)| {
                // add to accumulator
                acc.insert(
                    exchange,
                    pairs
                        .iter()
                        .map(|(pair, trades)| (*pair, trades.len()))
                        .collect::<FastHashMap<_, _>>(),
                );

                // extend trades
                for (pair, trades) in pairs {
                    self.0
                        .entry(exchange)
                        .or_default()
                        .entry(pair)
                        .or_default()
                        .extend(trades);
                }

                acc
            })
    }

    /// given the amount of entires per exchange per pair. removes
    /// the specified amount from the trade vector
    pub fn pop_historical_trades(
        &mut self,
        list: FastHashMap<CexExchange, FastHashMap<Pair, usize>>,
    ) {
        for (ex, pairs) in list {
            for (pair, offset) in pairs {
                let inner = self.0.entry(ex).or_default().entry(pair).or_default();
                inner.drain(0..offset);
            }
        }
    }
}

type ClickhouseTradeMap = Vec<(CexExchange, Vec<((String, String), Vec<RawCexTrades>)>)>;

impl<'de> Deserialize<'de> for CexTradeMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data: ClickhouseTradeMap = Deserialize::deserialize(deserializer)?;

        Ok(CexTradeMap(data.into_iter().fold(
            FastHashMap::default(),
            |mut acc: FastHashMap<CexExchange, FastHashMap<Pair, Vec<CexTrades>>>, (key, value)| {
                acc.entry(key).or_default().extend(value.into_iter().fold(
                    FastHashMap::default(),
                    |mut acc: FastHashMap<Pair, Vec<CexTrades>>, (pair, trades)| {
                        let pair = Pair(pair.0.parse().unwrap(), pair.1.parse().unwrap());
                        acc.entry(pair)
                            .or_default()
                            .extend(trades.into_iter().map(Into::into));
                        acc
                    },
                ));

                acc
            },
        )))
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive, Redefined)]
#[redefined(CexTradeMap)]
#[redefined_attr(
    to_source = "CexTradeMap::from_redefined(self.map)",
    from_source = "CexTradeMapRedefined::new(src.0)"
)]
pub struct CexTradeMapRedefined {
    pub map: Vec<(CexExchange, RedefinedTradeMapVec)>,
}

impl CexTradeMapRedefined {
    fn new(map: FastHashMap<CexExchange, FastHashMap<Pair, Vec<CexTrades>>>) -> Self {
        Self {
            map: map
                .into_iter()
                .map(|(exch, inner_map)| {
                    (
                        exch,
                        inner_map
                            .into_iter()
                            .map(|(a, b)| {
                                (
                                    PairRedefined::from_source(a),
                                    Vec::<CexTradesRedefined>::from_source(b),
                                )
                            })
                            .collect_vec(),
                    )
                })
                .collect::<Vec<_>>(),
        }
    }
}

implement_table_value_codecs_with_zc!(CexTradeMapRedefined);

#[derive(Debug, Clone, Serialize, Redefined, PartialEq, Eq)]
#[redefined_attr(derive(
    Debug,
    PartialEq,
    Eq,
    Clone,
    Hash,
    Serialize,
    rSerialize,
    rDeserialize,
    Archive
))]
pub struct CexTrades {
    #[redefined(same_fields)]
    pub exchange:  CexExchange,
    pub timestamp: u64,
    pub price:     Rational,
    pub amount:    Rational,
}

impl CexTrades {
    pub fn adjust_for_direction(&self, direction: Direction) -> Self {
        match direction {
            Direction::Buy => Self {
                exchange:  self.exchange,
                timestamp: self.timestamp,
                price:     self.price.clone(),
                amount:    &self.amount * &self.price,
            },
            Direction::Sell => Self {
                exchange:  self.exchange,
                timestamp: self.timestamp,
                price:     self.price.clone().reciprocal(),
                amount:    self.amount.clone(),
            },
        }
    }

    pub fn adjust_for_direction_mut(&mut self, direction: Direction) {
        match direction {
            Direction::Buy => self.amount *= &self.price,
            Direction::Sell => self.price.reciprocal_assign(),
        }
    }
}

impl From<RawCexTrades> for CexTrades {
    fn from(value: RawCexTrades) -> Self {
        Self {
            exchange:  value.exchange,
            price:     Rational::try_from_float_simplest(value.price).unwrap(),
            amount:    Rational::try_from_float_simplest(value.amount).unwrap(),
            timestamp: value.timestamp,
        }
    }
}
