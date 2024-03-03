use std::collections::HashMap;

use itertools::Itertools;
use malachite::{
    num::basic::traits::{One, Zero},
    Rational,
};

use super::cex::CexExchange;
use crate::pair::Pair;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PairTradeSide {
    /// token0 is base, token1 is quote
    pub pair: Pair,
    pub side: TradeSide,
}
impl PairTradeSide {
    pub fn new(pair: Pair, side: TradeSide) -> Self {
        Self { pair, side }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TradeSide {
    Buy,
    Sell,
}

// cex trades are sorted from least profitable to most profitable
pub struct CexTradeMap(HashMap<CexExchange, HashMap<PairTradeSide, Vec<CexTrades>>>);

impl CexTradeMap {
    pub fn get_vwam(
        &self,
        pair: Pair,
        side: TradeSide,
        volume: &Rational,
        // what execution quality we expect on a per exchange basis for the
        // given pair
        execution_quality_pct: Option<HashMap<CexExchange, HashMap<Pair, usize>>>,
    ) -> Rational {
        if let Some(pct) = execution_quality_pct {
        } else {
        }
        todo!()
    }

    pub fn get_fill(
        &self,
        trade: &PairTradeSide,
        volume: &Rational,
        quality: Option<HashMap<CexExchange, HashMap<Pair, usize>>>,
    ) {
    }

    pub fn get_fill_normal(
        &self,
        trade: &PairTradeSide,
        volume: &Rational,
        vol_overflow_am: &Rational,
        baskets: usize,
        quality: Option<HashMap<CexExchange, HashMap<Pair, usize>>>,
    ) -> Option<(Rational, Rational)> {
        let pair = trade.pair;
        let quality_pct = quality.map(|map| {
            map.into_iter()
                .map(|(k, v)| (k, v.get(&pair).copied().unwrap_or(100)))
                .collect::<HashMap<_, _>>()
        });

        let max_vol_per_trade = volume * vol_overflow_am;
        let trades = self
            .0
            .iter()
            .filter_map(|(exchange, trades)| {
                Some((
                    *exchange,
                    trades.get(&trade).map(|trades| {
                        trades
                            .into_iter()
                            .filter(|f| f.amount.le(&max_vol_per_trade))
                            .collect_vec()
                    })?,
                ))
            })
            .collect::<Vec<_>>();

        let mut trade_queue = PairTradeQueue::new(trade.side, trades, quality_pct);

        self.get_most_accurate_basket(trade_queue, volume, baskets)
    }

    pub fn get_most_accurate_basket(
        &self,
        mut queue: PairTradeQueue<'_>,
        volume: &Rational,
        baskets: usize,
    ) -> Option<(Rational, Rational)> {
        let mut trades = Vec::new();

        let volume_amount = volume * Rational::from(baskets);
        let mut cur_vol = Rational::ZERO;

        while volume_amount.gt(&cur_vol) {
            let Some(next) = queue.next_best_trade() else { break };
            cur_vol += &next.amount;
            trades.push(next);
        }

        let closest = closest(
            trades
                .iter()
                .combinations(2)
                .chain(trades.iter().combinations(3))
                .chain(trades.iter().combinations(4)),
            volume,
        )?;

        let mut vxp_maker = Rational::ZERO;
        let mut vxp_taker = Rational::ZERO;
        let mut trade_volume = Rational::ZERO;

        for trade in closest {
            let (m_fee, t_fee) = trade.exchange.fees();

            vxp_maker += (&trade.price * (Rational::ONE - m_fee)) * &trade.amount;
            vxp_taker += (&trade.price * (Rational::ONE - t_fee)) * &trade.amount;
            trade_volume += &trade.amount;
        }

        Some((vxp_maker / &trade_volume, vxp_taker / trade_volume))
    }

    pub fn get_fill_intermediary(
        &self,
        trade: &PairTradeSide,
        volume: &Rational,
        quality: Option<HashMap<CexExchange, HashMap<Pair, usize>>>,
    ) {
        // for (exchange, inner) in self.0.iter() {
        //     let intermediaries = exchange.most_common_quote_assets();
        //
        //     intermediaries.iter().filter_map(|&intermediary| {
        //         let pair1 = Pair(pair.0, intermediary);
        //         let pair2 = Pair(intermediary, pair.1);
        //
        //         // if let (Some(quote1), Some(quote2)) =
        //         //     (self.get_quote(&pair1, exchange),
        // self.get_quote(&pair2,         // exchange)) {
        //         //     let combined_price =
        //         //         (quote1.price.0 * quote2.price.0, quote1.price.1 *
        //         // quote2.price.1);     let combined_quote =
        //         // CexQuote {         exchange:  *exchange,
        //         //         timestamp: std::cmp::max(quote1.timestamp,
        //         // quote2.timestamp),         price:
        //         // combined_price,         token0:    pair.0,
        //         //     };
        //         //
        //         //     Some(combined_quote)
        //         // } else {
        //         //     None
        //         // }
        //     });
        // }
    }

    pub fn peek_top_trade(
        &self,
        trade: &PairTradeSide,
        exchange: &CexExchange,
    ) -> Option<&CexTrades> {
        self.0.get(exchange)?.get(trade)?.last()
    }
}

#[derive(Debug, Clone)]
pub struct CexTrades {
    pub timestamp: u64,
    pub exchange:  CexExchange,
    pub side:      TradeSide,
    pub price:     Rational,
    pub amount:    Rational,
}

pub struct PairTradeQueue<'a> {
    side:           TradeSide,
    exchange_depth: HashMap<CexExchange, usize>,
    trades:         Vec<(CexExchange, Vec<&'a CexTrades>)>,
}

impl<'a> PairTradeQueue<'a> {
    /// Assumes the trades are sorted based off the side that's passed in
    pub fn new(
        side: TradeSide,
        trades: Vec<(CexExchange, Vec<&'a CexTrades>)>,
        execution_quality_pct: Option<HashMap<CexExchange, usize>>,
    ) -> Self {
        // calculate the starting index based of the quality pct for the given exchange
        // and pair.
        let exchange_depth = if let Some(quality_pct) = execution_quality_pct {
            trades
                .iter()
                .map(|(exchange, data)| {
                    let length = data.len();
                    let quality = quality_pct.get(exchange).copied().unwrap_or(100);
                    let idx = length - (length * quality / 100);
                    (*exchange, idx)
                })
                .collect::<HashMap<_, _>>()
        } else {
            HashMap::default()
        };

        Self { side, exchange_depth, trades }
    }

    pub fn next_best_trade(&mut self) -> Option<CexTrades> {
        let mut next: Option<CexTrades> = None;

        for (exchange, trades) in &self.trades {
            let exchange_depth = *self.exchange_depth.entry(*exchange).or_insert(0);
            let len = trades.len() - 1;

            // hit max depth
            if exchange_depth > len {
                continue
            }

            if let Some(trade) = trades.get(len - exchange_depth) {
                if let Some(cur_best) = next.as_ref() {
                    match self.side {
                        TradeSide::Buy => {
                            if trade.price < cur_best.price {
                                next = Some(trade.clone().clone());
                            }
                        }
                        TradeSide::Sell => {
                            if trade.price > cur_best.price {
                                next = Some(trade.clone().clone());
                            }
                        }
                    }
                // not set
                } else {
                    next = Some(trade.clone().clone());
                }
            }
        }

        // increment ptr
        if let Some(next) = next.as_ref() {
            *self.exchange_depth.get_mut(&next.exchange).unwrap() += 1;
        }

        next
    }
}

pub fn closest<'a>(
    iter: impl Iterator<Item = Vec<&'a CexTrades>>,
    vol: &Rational,
) -> Option<Vec<&'a CexTrades>> {
    iter.sorted_by(|a, b| {
        b.iter()
            .map(|t| &t.amount)
            .sum::<Rational>()
            .cmp(&a.iter().map(|t| &t.amount).sum::<Rational>())
    })
    .find(|set| set.iter().map(|t| &t.amount).sum::<Rational>().ge(vol))
}
