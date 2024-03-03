use std::{cmp::max, collections::HashMap};

use alloy_primitives::Address;
use itertools::Itertools;
use malachite::{
    num::basic::traits::{One, Zero},
    Rational,
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use super::cex::CexExchange;
use crate::pair::Pair;

/// TODO: lets prob not set this to 100%
const BASE_EXECUTION_QUALITY: usize = 100;
/// The amount of excess volume a trade can do to be considered
/// as part of execution
const EXCESS_VOLUME_PCT: Rational = Rational::const_from_unsigneds(5, 100);

type MakerTaker = (Rational, Rational);

// cex trades are sorted from lowest fill price to highest fill price
pub struct CexTradeMap(HashMap<CexExchange, HashMap<Pair, Vec<CexTrades>>>);

type FoldVWAM = HashMap<Address, Vec<MakerTaker>>;

impl CexTradeMap {
    /// takes the best vwam price for
    pub fn get_vwam_price(
        &self,
        pair: &Pair,
        volume: &Rational,
        baskets: usize,
        quality: Option<HashMap<CexExchange, HashMap<Pair, usize>>>,
    ) -> Option<MakerTaker> {
        let regular = self.get_vwam_no_intermediary(pair, volume, baskets, quality.as_ref());
        let inter = self.get_vwam_via_intermediary(pair, volume, baskets, quality.as_ref());

        match (regular, inter) {
            (Some(reg), Some(inter)) => Some((max(reg.0, inter.0), max(reg.1, inter.1))),
            (Some(reg), None) => Some(reg),
            (None, Some(inter)) => Some(inter),
            _ => None,
        }
    }

    fn get_vwam_via_intermediary(
        &self,
        pair: &Pair,
        volume: &Rational,
        baskets: usize,
        quality: Option<&HashMap<CexExchange, HashMap<Pair, usize>>>,
    ) -> Option<MakerTaker> {
        let fold_fn = |(mut pair0_vwam, mut pair1_vwam): (FoldVWAM, FoldVWAM), (iter_0, iter_1)| {
            for (k, v) in iter_0 {
                pair0_vwam.entry(k).or_insert(vec![]).extend(v);
            }
            for (k, v) in iter_1 {
                pair1_vwam.entry(k).or_insert(vec![]).extend(v);
            }

            (pair0_vwam, pair1_vwam)
        };

        let (pair0_vwams, mut pair1_vwams) = self
            .0
            .keys()
            .map(|exchange| {
                exchange
                    .most_common_quote_assets()
                    .into_par_iter()
                    .filter_map(|intermediary| {
                        let pair0 = Pair(pair.0, intermediary);
                        let pair1 = Pair(intermediary, pair.1);
                        Some((
                            (
                                intermediary,
                                self.get_vwam_no_intermediary(&pair0, volume, baskets, quality)?,
                            ),
                            (
                                intermediary,
                                self.get_vwam_no_intermediary(&pair1, volume, baskets, quality)?,
                            ),
                        ))
                    })
                    .fold(
                        || (HashMap::new(), HashMap::new()),
                        |(mut pair0_vwam, mut pair1_vwam), ((iter0, prices0), (iter1, prices1))| {
                            pair0_vwam.entry(iter0).or_insert(vec![]).push(prices0);
                            pair1_vwam.entry(iter1).or_insert(vec![]).push(prices1);
                            (pair0_vwam, pair1_vwam)
                        },
                    )
                    .reduce(|| (HashMap::new(), HashMap::new()), fold_fn)
            })
            .fold((HashMap::new(), HashMap::new()), fold_fn);

        let (maker, taker): (Vec<_>, Vec<_>) = pair0_vwams
            .into_iter()
            .flat_map(|(inter, vwam0)| {
                let Some(vwam1) = pair1_vwams.remove(&inter) else { return vec![] };

                vwam0
                    .into_iter()
                    .flat_map(|(maker0, taker0)| {
                        vwam1
                            .iter()
                            .map(move |(maker1, taker1)| (&maker0 * maker1, &taker0 * taker1))
                    })
                    .collect_vec()
            })
            .unzip();

        Some((maker.into_iter().max()?, taker.into_iter().max()?))
    }

    fn get_vwam_no_intermediary(
        &self,
        pair: &Pair,
        volume: &Rational,
        baskets: usize,
        quality: Option<&HashMap<CexExchange, HashMap<Pair, usize>>>,
    ) -> Option<MakerTaker> {
        let quality_pct = quality.map(|map| {
            map.into_iter()
                .map(|(k, v)| (*k, v.get(pair).copied().unwrap_or(BASE_EXECUTION_QUALITY)))
                .collect::<HashMap<_, _>>()
        });

        let max_vol_per_trade = volume + (volume * EXCESS_VOLUME_PCT);
        let trades = self
            .0
            .iter()
            .filter_map(|(exchange, trades)| {
                Some((
                    *exchange,
                    trades.get(pair).map(|trades| {
                        trades
                            .into_iter()
                            .filter(|f| f.amount.le(&max_vol_per_trade))
                            .collect_vec()
                    })?,
                ))
            })
            .collect::<Vec<_>>();

        let trade_queue = PairTradeQueue::new(trades, quality_pct);
        self.get_most_accurate_basket(trade_queue, volume, baskets)
    }

    fn get_most_accurate_basket(
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
            trades.push(next.clone());
        }

        let closest = closest(
            trades.iter().map(|t| vec![t]).chain(
                trades
                    .iter()
                    .combinations(2)
                    .chain(trades.iter().combinations(3))
                    .chain(trades.iter().combinations(4)),
            ),
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
}

#[derive(Debug, Clone)]
pub struct CexTrades {
    pub timestamp: u64,
    pub exchange:  CexExchange,
    pub price:     Rational,
    pub amount:    Rational,
}

/// Its ok that we create 2 of these for pair price and intermediary price
/// as it runs off of borrowed data so there is no overhead we occur
pub struct PairTradeQueue<'a> {
    exchange_depth: HashMap<CexExchange, usize>,
    trades:         Vec<(CexExchange, Vec<&'a CexTrades>)>,
}

impl<'a> PairTradeQueue<'a> {
    /// Assumes the trades are sorted based off the side that's passed in
    pub fn new(
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

        Self { exchange_depth, trades }
    }

    pub fn next_best_trade(&mut self) -> Option<&CexTrades> {
        let mut next: Option<&CexTrades> = None;

        for (exchange, trades) in &self.trades {
            let exchange_depth = *self.exchange_depth.entry(*exchange).or_insert(0);
            let len = trades.len() - 1;

            // hit max depth
            if exchange_depth > len {
                continue
            }

            if let Some(trade) = trades.get(len - exchange_depth) {
                if let Some(cur_best) = next.as_ref() {
                    // found a better price
                    if trade.price > cur_best.price {
                        next = Some(*trade)
                    }
                // not set
                } else {
                    next = Some(*trade);
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
    // sort from lowest to highest volume returning the first
    iter.sorted_by(|a, b| {
        a.iter()
            .map(|t| &t.amount)
            .sum::<Rational>()
            .cmp(&b.iter().map(|t| &t.amount).sum::<Rational>())
    })
    .find(|set| set.iter().map(|t| &t.amount).sum::<Rational>().ge(vol))
}
