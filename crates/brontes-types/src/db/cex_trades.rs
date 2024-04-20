use core::f64;
use std::{cmp::max, f64::consts::E, fmt::Display, marker::PhantomData};

use alloy_primitives::Address;
use clickhouse::Row;
use itertools::Itertools;
use malachite::{
    num::basic::traits::{One, Zero},
    Rational,
};
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use redefined::{Redefined, RedefinedConvert};
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};

use super::{cex::CexExchange, raw_cex_trades::RawCexTrades};
use crate::{
    db::redefined_types::malachite::RationalRedefined,
    implement_table_value_codecs_with_zc,
    pair::{Pair, PairRedefined},
    utils::ToFloatNearest,
    FastHashMap, FastHashSet,
};

/// TODO: lets prob not set this to 100%
const BASE_EXECUTION_QUALITY: usize = 45;
/// The amount of excess volume a trade can do to be considered
/// as part of execution
const EXCESS_VOLUME_PCT: Rational = Rational::const_from_unsigneds(10, 100);

/// the calculated price based off of trades with the estimated exchanges with
/// volume amount that where used to hedge
#[derive(Debug, Clone)]
pub struct ExchangePrice {
    // cex exchange with amount of volume executed on it
    pub exchanges: Vec<(CexExchange, Rational)>,
    pub price:     Rational,
}

impl Display for ExchangePrice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{:#?}", self.exchanges)?;
        writeln!(f, "{}", self.price.clone().to_float())
    }
}

type MakerTaker = (ExchangePrice, ExchangePrice);
type RedefinedTradeMapVec = Vec<(PairRedefined, Vec<CexTradesRedefined>)>;

const PRE_DECAY: f64 = -0.5;
const POST_DECAY: f64 = -0.2;

// trades sorted by time-stamp with the index to block time-stamp closest to the
// block_number
pub struct CexTradeMap2(pub FastHashMap<CexExchange, FastHashMap<Pair, (usize, Vec<CexTrades>)>>);

impl CexTradeMap2 {
    pub fn get_price(
        &self,
        exchanges: &[CexExchange],
        pair: Pair,
        volume: &Rational,
        timestamp: u128,
    ) -> Option<(Rational, Rational)> {
        if pair.0 == pair.1 {
            return Some((Rational::ZERO, Rational::ZERO))
        }

        let res = self
            .get_vwam_price(exchanges, pair, volume, timestamp)
            .or_else(|| self.get_vwam_price_via_intermediary(exchanges, &pair, volume, timestamp));

        if res.is_none() {
            tracing::debug!(?pair, "no vwam found");
        }

        res
    }

    fn get_vwam_price_via_intermediary(
        &self,
        exchanges: &[CexExchange],
        pair: &Pair,
        volume: &Rational,
        timestamp: u128,
    ) -> Option<(Rational, Rational)> {
        self.calculate_intermediary_addresses(exchanges, pair)
            .into_par_iter()
            .filter_map(|intermediary| {
                // usdc / bnb 0.004668534080298786price
                let pair0 = Pair(pair.0, intermediary);
                // bnb / eth 0.1298price
                let pair1 = Pair(intermediary, pair.1);
                // check if we have a path
                let mut has_pair0 = false;
                let mut has_pair1 = false;

                for (_, trades) in self.0.iter().filter(|(ex, _)| exchanges.contains(ex)) {
                    has_pair0 |= trades.contains_key(&pair0);
                    has_pair1 |= trades.contains_key(&pair1);

                    if has_pair1 && has_pair0 {
                        break
                    }
                }

                if !(has_pair0 && has_pair1) {
                    return None
                }

                tracing::debug!(?pair, ?intermediary, "trying via intermediary");
                let res = self.get_vwam_price(exchanges, pair0, volume, timestamp)?;
                let new_vol = volume * &res.0;
                let pair1_v = self.get_vwam_price(exchanges, pair1, &new_vol, timestamp)?;

                let maker = res.0 * pair1_v.0;
                let taker = res.1 * pair1_v.1;

                Some((maker, taker))
            })
            .max_by_key(|a| a.0.clone())
    }

    fn get_vwam_price(
        &self,
        exchanges: &[CexExchange],
        pair: Pair,
        vol: &Rational,
        timestamp: u128,
    ) -> Option<(Rational, Rational)> {
        let (ptrs, trades): (FastHashMap<CexExchange, (usize, usize)>, Vec<(CexExchange, _)>) =
            self.0
                .iter()
                .filter(|(e, _)| exchanges.contains(e))
                .filter_map(|(exchange, trades)| Some((*exchange, trades.get(&pair)?)))
                .map(|(ex, (idx, trades))| ((ex, (idx + 1, *idx)), (ex, trades)))
                .unzip();

        let mut walker =
            PairTradeWalker::new(trades, ptrs, timestamp - 500_000, timestamp + 2_000_000);

        let mut vxp_maker = Rational::ZERO;
        let mut vxp_taker = Rational::ZERO;
        let mut trade_volume = Rational::ZERO;

        while trade_volume.le(vol) {
            for trade in walker.get_trades_for_window() {
                let (m_fee, t_fee) = trade.get().exchange.fees();
                let weight = calcuate_weight(timestamp as u64, trade.get().timestamp as u64);

                vxp_maker +=
                    (&trade.get().price * (Rational::ONE - m_fee)) * &trade.get().amount * &weight;
                vxp_taker +=
                    (&trade.get().price * (Rational::ONE - t_fee)) * &trade.get().amount * &weight;
                trade_volume += &trade.get().amount * weight;
            }

            if walker.get_min_time_delta(timestamp) >= 3_000_000
                || walker.get_max_time_delta(timestamp) >= 5_000_000
            {
                break
            }

            let min_expand =
                if walker.get_max_time_delta(timestamp) >= 3_000_000 { 100_000 } else { 0 };

            walker.expand_time_bounds(min_expand, 100_000);
        }

        if &trade_volume < vol {
            return None
        }

        Some((vxp_maker, vxp_taker))
    }

    fn calculate_intermediary_addresses(
        &self,
        exchanges: &[CexExchange],
        pair: &Pair,
    ) -> FastHashSet<Address> {
        self.0
            .par_iter()
            .filter(|(k, _)| exchanges.contains(k))
            .flat_map(|(_, pairs)| {
                pairs
                    .keys()
                    .filter_map(|trade_pair| {
                        if trade_pair.ordered() == pair.ordered() {
                            return None
                        }

                        (trade_pair.0 == pair.0)
                            .then_some(trade_pair.1)
                            .or_else(|| (trade_pair.1 == pair.1).then_some(trade_pair.0))
                    })
                    .collect_vec()
            })
            .collect::<FastHashSet<_>>()
    }
}

fn calcuate_weight(block_time: u64, trade_time: u64) -> Rational {
    let pre = trade_time < block_time;

    Rational::try_from_float_simplest(if pre {
        E.powf(PRE_DECAY * (block_time - trade_time) as f64)
    } else {
        E.powf(POST_DECAY * (trade_time - block_time) as f64)
    })
    .unwrap()
}

pub struct PairTradeWalker<'a> {
    min_timestamp: u128,
    max_timestamp: u128,
    exchange_ptrs: FastHashMap<CexExchange, (usize, usize)>,
    trades:        Vec<(CexExchange, &'a Vec<CexTrades>)>,
}

impl<'a> PairTradeWalker<'a> {
    pub fn new(
        trades: Vec<(CexExchange, &'a Vec<CexTrades>)>,
        exchange_ptrs: FastHashMap<CexExchange, (usize, usize)>,
        min_timestamp: u128,
        max_timestamp: u128,
    ) -> Self {
        Self { max_timestamp, min_timestamp, trades, exchange_ptrs }
    }

    fn get_min_time_delta(&self, timestamp: u128) -> u128 {
        timestamp - self.min_timestamp
    }

    fn get_max_time_delta(&self, timestamp: u128) -> u128 {
        self.max_timestamp - timestamp
    }

    fn expand_time_bounds(&mut self, min: u128, max: u128) {
        self.min_timestamp -= min;
        self.max_timestamp += max;
    }

    fn get_trades_for_window(&mut self) -> Vec<CexTradePtr<'a>> {
        let mut trade_res: Vec<CexTradePtr<'a>> = Vec::with_capacity(420);

        for (exchange, trades) in &self.trades {
            let Some((lower_idx, upper_idx)) = self.exchange_ptrs.get_mut(&exchange) else {
                continue
            };

            // add lower
            if *lower_idx > 0 {
                loop {
                    let next_trade = &trades[*lower_idx - 1];
                    if next_trade.timestamp >= self.min_timestamp {
                        trade_res.push(CexTradePtr::new(next_trade));
                        *lower_idx -= 1;
                    } else {
                        break
                    }

                    if *lower_idx == 0 {
                        break
                    }
                }
            }

            let max = trades.len() - 1;
            if *upper_idx < max {
                loop {
                    let next_trade = &trades[*upper_idx + 1];
                    if next_trade.timestamp <= self.max_timestamp {
                        trade_res.push(CexTradePtr::new(next_trade));
                        *upper_idx += 1;
                    } else {
                        break
                    }

                    if *upper_idx == max {
                        break
                    }
                }
            }
        }

        trade_res
    }
}

// cex trades are sorted from lowest fill price to highest fill price
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
                                acc.entry(pair.to_source()).or_default().extend(
                                    trades
                                        .into_iter()
                                        .map(|t| t.to_source())
                                        .sorted_unstable_by_key(|a| a.price.clone()),
                                );
                                acc
                            },
                        ),
                    )
                })
                .collect(),
        )
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
                        acc.entry(pair).or_default().extend(
                            trades
                                .into_iter()
                                .map(Into::into)
                                .sorted_unstable_by_key(|a: &CexTrades| a.price.clone()),
                        );
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

type FoldVWAM = FastHashMap<Address, Vec<MakerTakerWithVolumeFilled>>;

impl CexTradeMap {
    // Calculates VWAPs for the given pair across all provided exchanges - this
    // will assess trades across each exchange
    //
    // For non-intermediary dependent pairs, we do the following:
    // - 1. Adjust each exchange's trade set by the assumed execution quality for
    //   the given pair on the exchange. We assess a larger percentage of trades if
    //   execution quality is assumed to be lower.
    // - 2. Exclude trades with a volume that is too large to be considered
    //   potential hedging trades.
    // - 3. Order all trades for each exchange by price.
    // - 4. Finally, we pick a vector of trades whose total volume is closest to the
    //   swap volume.
    // - 5. Calculate the VWAP for the chosen set of trades.

    // For non-intermediary dependant pairs
    // - 1. Calculate VWAPs for all potential intermediary pairs (using above
    //   process)
    // -- Pair's with insufficient volume will be filtered out here which will
    // filter the route in the next step
    // - 2. Combines VWAP's to assess potential routes
    // - 3. Selects most profitable route and returns it as the Price
    // -- It should be noted here that this will not aggregate multiple possible
    // routes
    pub fn get_price(
        &self,
        exchanges: &[CexExchange],
        pair: &Pair,
        volume: &Rational,
        quality: Option<FastHashMap<CexExchange, FastHashMap<Pair, usize>>>,
    ) -> Option<MakerTaker> {
        if pair.0 == pair.1 {
            return Some((
                ExchangePrice { exchanges: vec![], price: Rational::ONE },
                ExchangePrice { exchanges: vec![], price: Rational::ONE },
            ))
        }

        let res = self
            .get_vwam_no_intermediary(exchanges, pair, volume, quality.as_ref())
            .or_else(|| self.get_vwam_via_intermediary(exchanges, pair, volume, quality.as_ref()));

        if res.is_none() {
            tracing::debug!(?pair, "no vwam found");
        }

        res
    }

    fn calculate_intermediary_addresses(
        &self,
        exchanges: &[CexExchange],
        pair: &Pair,
    ) -> FastHashSet<Address> {
        self.0
            .par_iter()
            .filter(|(k, _)| exchanges.contains(k))
            .flat_map(|(_, pairs)| {
                pairs
                    .keys()
                    .filter_map(|trade_pair| {
                        if trade_pair.ordered() == pair.ordered() {
                            return None
                        }

                        (trade_pair.0 == pair.0)
                            .then_some(trade_pair.1)
                            .or_else(|| (trade_pair.1 == pair.1).then_some(trade_pair.0))
                    })
                    .collect_vec()
            })
            .collect::<FastHashSet<_>>()
    }

    fn get_vwam_via_intermediary(
        &self,
        exchanges: &[CexExchange],
        pair: &Pair,
        volume: &Rational,
        quality: Option<&FastHashMap<CexExchange, FastHashMap<Pair, usize>>>,
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

        let (pair0_vwams, pair1_vwams) = self
            .calculate_intermediary_addresses(exchanges, pair)
            .into_par_iter()
            .filter_map(|intermediary| {
                // usdc / bnb 0.004668534080298786price
                let pair0 = Pair(pair.0, intermediary);
                // bnb / eth 0.1298price
                let pair1 = Pair(intermediary, pair.1);
                // check if we have a path
                let mut has_pair0 = false;
                let mut has_pair1 = false;
                for (_, trades) in self.0.iter().filter(|(ex, _)| exchanges.contains(ex)) {
                    has_pair0 |= trades.contains_key(&pair0);
                    has_pair1 |= trades.contains_key(&pair1);

                    if has_pair1 && has_pair0 {
                        break
                    }
                }

                if !(has_pair0 && has_pair1) {
                    return None
                }

                tracing::debug!(?pair, ?intermediary, "trying via intermediary");

                let (i, res) = (
                    intermediary,
                    self.get_vwam_via_intermediary_spread(exchanges, &pair0, volume, quality)?,
                );

                let new_vol = volume * &res.prices.0.price;

                Some((
                    (i, res),
                    (
                        intermediary,
                        self.get_vwam_via_intermediary_spread(
                            exchanges, &pair1, &new_vol, quality,
                        )?,
                    ),
                ))
            })
            .fold(
                || (FastHashMap::default(), FastHashMap::default()),
                |(mut pair0_vwam, mut pair1_vwam), ((iter0, prices0), (iter1, prices1))| {
                    pair0_vwam.entry(iter0).or_insert(vec![]).push(prices0);
                    pair1_vwam.entry(iter1).or_insert(vec![]).push(prices1);
                    (pair0_vwam, pair1_vwam)
                },
            )
            .reduce(|| (FastHashMap::default(), FastHashMap::default()), fold_fn);

        // calculates best possible mult cross pair price
        calculate_multi_cross_pair(pair0_vwams, pair1_vwams, volume)
    }

    pub fn get_vwam_via_intermediary_spread(
        &self,
        exchanges: &[CexExchange],
        pair: &Pair,
        volume: &Rational,
        quality: Option<&FastHashMap<CexExchange, FastHashMap<Pair, usize>>>,
    ) -> Option<MakerTakerWithVolumeFilled> {
        // Populate Map of Assumed Execution Quality by Exchange
        // - We're making the assumption that the stat arber isn't hitting *every* good
        //   markout for each pair on each exchange.
        // - Quality percent adjusts the total percent of "good" trades the arber is
        //   capturing for the relevant pair on a given exchange.
        let quality_pct = quality.map(|map| {
            map.iter()
                .map(|(k, v)| (*k, v.get(pair).copied().unwrap_or(BASE_EXECUTION_QUALITY)))
                .collect::<FastHashMap<_, _>>()
        });

        // Filter Exchange Trades Based On Volume
        // - This filters trades used to calculate the VWAM by excluding trades that
        //   have significantly more volume than the needed inventory offset
        // - The assumption here is the stat arber is trading just for this arb and
        //   isn't offsetting inventory for other purposes at the same time

        let max_vol_per_trade = volume + (volume * EXCESS_VOLUME_PCT);
        let trades = self
            .0
            .iter()
            .filter(|(e, _)| exchanges.contains(e))
            .filter_map(|(exchange, trades)| {
                let result = trades.get(pair).map(|trades| {
                    trades
                        .iter()
                        .filter(|f| f.amount.le(&max_vol_per_trade))
                        .collect_vec()
                });

                Some((*exchange, result?))
            })
            .collect::<Vec<_>>();

        if trades.is_empty() {
            return None
        }
        // Populate trade queue per exchange
        // - This utilizes the quality percent number to set the number of trades that
        //   will be assessed in picking a bucket to calculate the vwam with. A lower
        //   quality percent will cause us to examine more trades (go deeper into the
        //   vec) - resulting in a potentially worse price (remember, trades are sorted
        //   by price)
        let trade_queue = PairTradeQueue::new(trades, quality_pct);
        self.get_most_accurate_basket_intermediary(trade_queue, volume)
    }

    fn get_vwam_no_intermediary(
        &self,
        exchanges: &[CexExchange],
        pair: &Pair,
        volume: &Rational,
        quality: Option<&FastHashMap<CexExchange, FastHashMap<Pair, usize>>>,
    ) -> Option<MakerTaker> {
        // Populate Map of Assumed Execution Quality by Exchange
        // - We're making the assumption that the stat arber isn't hitting *every* good
        //   markout for each pair on each exchange.
        // - Quality percent adjusts the total percent of "good" trades the arber is
        //   capturing for the relevant pair on a given exchange.
        let quality_pct = quality.map(|map| {
            map.iter()
                .map(|(k, v)| (*k, v.get(pair).copied().unwrap_or(BASE_EXECUTION_QUALITY)))
                .collect::<FastHashMap<_, _>>()
        });

        // Filter Exchange Trades Based On Volume
        // - This filters trades used to calculate the VWAM by excluding trades that
        //   have significantly more volume than the needed inventory offset
        // - The assumption here is the stat arber is trading just for this arb and
        //   isn't offsetting inventory for other purposes at the same time

        let max_vol_per_trade = volume + (volume * EXCESS_VOLUME_PCT);
        let trades = self
            .0
            .iter()
            .filter(|(e, _)| exchanges.contains(e))
            .filter_map(|(exchange, trades)| {
                Some((
                    *exchange,
                    trades.get(pair).map(|trades| {
                        trades
                            .iter()
                            .filter(|f| f.amount.le(&max_vol_per_trade))
                            .collect_vec()
                    })?,
                ))
            })
            .collect::<Vec<_>>();

        if trades.is_empty() {
            return None
        }
        // Populate trade queue per exchange
        // - This utilizes the quality percent number to set the number of trades that
        //   will be assessed in picking a bucket to calculate the vwam with. A lower
        //   quality percent will cause us to examine more trades (go deeper into the
        //   vec) - resulting in a potentially worse price (remember, trades are sorted
        //   by price)
        let trade_queue = PairTradeQueue::new(trades, quality_pct);

        self.get_most_accurate_basket(trade_queue, volume)
    }

    fn get_most_accurate_basket_intermediary(
        &self,
        mut queue: PairTradeQueue<'_>,
        volume: &Rational,
    ) -> Option<MakerTakerWithVolumeFilled> {
        let mut trades = Vec::new();

        // multiply volume by baskets to assess more potential baskets of trades
        let volume_amount = volume;
        let mut cur_vol = Rational::ZERO;

        // Populates an ordered vec of trades from best to worst price
        while volume_amount.gt(&cur_vol) {
            let Some(next) = queue.next_best_trade() else {
                break;
            };
            // we do this due to the sheer amount of trades we have and to not have to copy.
            // all of this is safe
            cur_vol += &next.get().amount;

            trades.push(next);
        }

        let mut vxp_maker = Rational::ZERO;
        let mut vxp_taker = Rational::ZERO;
        let mut trade_volume = Rational::ZERO;
        let mut exchange_with_vol = FastHashMap::default();

        // For the closest basket sum volume and volume weighted prices
        for trade in trades {
            let (m_fee, t_fee) = trade.get().exchange.fees();

            vxp_maker += (&trade.get().price * (Rational::ONE - m_fee)) * &trade.get().amount;
            vxp_taker += (&trade.get().price * (Rational::ONE - t_fee)) * &trade.get().amount;

            *exchange_with_vol
                .entry(trade.get().exchange)
                .or_insert(Rational::ZERO) += &trade.get().amount;

            trade_volume += &trade.get().amount;
        }

        if trade_volume == Rational::ZERO {
            return None
        }
        let exchanges = exchange_with_vol.into_iter().collect_vec();

        let maker =
            ExchangePrice { exchanges: exchanges.clone(), price: vxp_maker / &trade_volume };
        let taker =
            ExchangePrice { exchanges: exchanges.clone(), price: vxp_taker / &trade_volume };

        Some(MakerTakerWithVolumeFilled {
            volume_looked_at: cur_vol,
            prices:           (maker, taker),
        })
    }

    fn get_most_accurate_basket(
        &self,
        mut queue: PairTradeQueue<'_>,
        volume: &Rational,
    ) -> Option<MakerTaker> {
        let mut trades = Vec::new();

        // multiply volume by baskets to assess more potential baskets of trades
        let volume_amount = volume;
        let mut cur_vol = Rational::ZERO;

        // Populates an ordered vec of trades from best to worst price
        while volume_amount.gt(&cur_vol) {
            let Some(next) = queue.next_best_trade() else {
                break;
            };
            // we do this due to the sheer amount of trades we have and to not have to copy.
            // all of this is safe
            cur_vol += &next.get().amount;

            trades.push(next);
        }

        if &cur_vol < volume {
            return None
        }

        // // Groups trades into a set of iterators, first including all trades, then
        // all // combinations of 2 trades, then all combinations of 3 trades,
        // then all // combinations of 4 trades
        // // - The assumption here is we don't frequently need to evaluate more than a
        // set //   of 4 trades
        // //TODO: Bench & check if it's it worth to parallelize
        // let trade_buckets_iterator = trades
        //     .iter()
        //     .map(|t| vec![t])
        //     .chain(
        //         trades
        //             .iter()
        //             .combinations(2)
        //             .chain(trades.iter().combinations(3)),
        //     )
        //     .collect::<Vec<_>>();
        // // Gets the vec of trades that's closest to the volume of the stat arb swap
        // // Will not return a vec that does not have enough volume to fill the arb
        // let closest = closest(trade_buckets_iterator.into_par_iter(), volume)?;

        let mut vxp_maker = Rational::ZERO;
        let mut vxp_taker = Rational::ZERO;
        let mut trade_volume = Rational::ZERO;
        let mut exchange_with_vol = FastHashMap::default();

        // For the closest basket sum volume and volume weighted prices
        for trade in trades {
            let (m_fee, t_fee) = trade.get().exchange.fees();

            vxp_maker += (&trade.get().price * (Rational::ONE - m_fee)) * &trade.get().amount;
            vxp_taker += (&trade.get().price * (Rational::ONE - t_fee)) * &trade.get().amount;

            *exchange_with_vol
                .entry(trade.get().exchange)
                .or_insert(Rational::ZERO) += &trade.get().amount;

            trade_volume += &trade.get().amount;
        }

        if trade_volume == Rational::ZERO {
            return None
        }
        let exchanges = exchange_with_vol.into_iter().collect_vec();

        let maker =
            ExchangePrice { exchanges: exchanges.clone(), price: vxp_maker / &trade_volume };
        let taker =
            ExchangePrice { exchanges: exchanges.clone(), price: vxp_taker / &trade_volume };

        Some((maker, taker))
    }
}

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
    pub timestamp: u128,
    pub price:     Rational,
    pub amount:    Rational,
}

impl From<RawCexTrades> for CexTrades {
    fn from(value: RawCexTrades) -> Self {
        Self {
            exchange:  value.exchange,
            price:     Rational::try_from_float_simplest(value.price).unwrap(),
            amount:    Rational::try_from_float_simplest(value.amount).unwrap(),
            timestamp: value.timestamp as u128,
        }
    }
}

/// Its ok that we create 2 of these for pair price and intermediary price
/// as it runs off of borrowed data so there is no overhead we occur
pub struct PairTradeQueue<'a> {
    exchange_depth: FastHashMap<CexExchange, usize>,
    trades:         Vec<(CexExchange, Vec<&'a CexTrades>)>,
}

impl<'a> PairTradeQueue<'a> {
    /// Assumes the trades are sorted based off the side that's passed in
    pub fn new(
        trades: Vec<(CexExchange, Vec<&'a CexTrades>)>,
        execution_quality_pct: Option<FastHashMap<CexExchange, usize>>,
    ) -> Self {
        // calculate the ending index (depth) based of the quality pct for the given
        // exchange and pair.
        // -- quality percent is the assumed percent of good trades the arber is
        // capturing for the relevant pair on a given exchange
        // -- a lower quality percentage means we need to assess more trades because
        // it's possible the arber was getting bad execution. Vice versa for a
        // high quality percent
        let exchange_depth = if let Some(quality_pct) = execution_quality_pct {
            trades
                .iter()
                .map(|(exchange, data)| {
                    let length = data.len();
                    let quality = quality_pct.get(exchange).copied().unwrap_or(100);
                    let idx = length - (length * quality / 100);
                    (*exchange, idx)
                })
                .collect::<FastHashMap<_, _>>()
        } else {
            FastHashMap::default()
        };

        Self { exchange_depth, trades }
    }

    fn next_best_trade(&mut self) -> Option<CexTradePtr<'a>> {
        let mut next: Option<CexTradePtr<'a>> = None;

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

                    if trade.price.gt(&cur_best.get().price) {
                        next = Some(CexTradePtr::new(trade));
                    }
                // not set
                } else {
                    next = Some(CexTradePtr::new(trade));
                }
            }
        }

        // increment ptr
        if let Some(next) = next.as_ref() {
            *self.exchange_depth.get_mut(&next.get().exchange).unwrap() += 1;
        }

        next
    }
}

/// take all intermediaries that we have collected, convert them into the full
/// pair price with the amount of volume that they have cleared. We then will
/// take the best price up-to our target volume and do a weighted average of the
/// best.
fn calculate_multi_cross_pair(
    v0: FastHashMap<Address, Vec<MakerTakerWithVolumeFilled>>,
    mut v1: FastHashMap<Address, Vec<MakerTakerWithVolumeFilled>>,
    v0_volume_needed: &Rational,
) -> Option<MakerTaker> {
    let mut total_volume_pct = Rational::ZERO;

    let (wi, mut maker, mut taker) = v0
        .into_iter()
        .flat_map(|(inter, vwam0)| {
            let Some(vwam1) = v1.remove(&inter) else {
                return vec![];
            };

            let res = vwam0
                .into_iter()
                .flat_map(|first_vwam| {
                    vwam1.iter().map(move |second_vwam| {
                        let maker_exchanges = first_vwam
                            .prices
                            .0
                            .exchanges
                            .iter()
                            .chain(second_vwam.prices.0.exchanges.iter())
                            .fold(FastHashMap::default(), |mut a, b| {
                                *a.entry(b.0).or_insert(Rational::ZERO) += &b.1;
                                a
                            })
                            .into_iter()
                            .collect_vec();

                        let taker_exchanges = first_vwam
                            .prices
                            .1
                            .exchanges
                            .iter()
                            .chain(second_vwam.prices.1.exchanges.iter())
                            .fold(FastHashMap::default(), |mut a, b| {
                                *a.entry(b.0).or_insert(Rational::ZERO) += &b.1;
                                a
                            })
                            .into_iter()
                            .collect_vec();

                        let v1_volume_needed = &first_vwam.prices.0.price * v0_volume_needed;

                        // we take the lower end of the volume filled for better accuracy
                        let total_volume_pct_0 = &first_vwam.volume_looked_at / v0_volume_needed;
                        let total_volume_pct_1 = &second_vwam.volume_looked_at / &v1_volume_needed;

                        let volume_pct = max(total_volume_pct_0, total_volume_pct_1);

                        let maker = ExchangePrice {
                            exchanges: maker_exchanges,
                            price:     &first_vwam.prices.0.price * &second_vwam.prices.0.price,
                        };

                        let taker = ExchangePrice {
                            exchanges: taker_exchanges,
                            price:     &first_vwam.prices.1.price * &second_vwam.prices.1.price,
                        };

                        (volume_pct, maker, taker)
                    })
                })
                .collect_vec();

            res
        })
        .sorted_by(|(_, a, _), (_, b, _)| b.price.cmp(&a.price))
        .take_while(|(volume_pct, ..)| {
            total_volume_pct += volume_pct;
            total_volume_pct < Rational::ONE
        })
        .fold(
            (
                Rational::ZERO,
                ExchangePrice { price: Rational::ZERO, exchanges: vec![] },
                ExchangePrice { price: Rational::ZERO, exchanges: vec![] },
            ),
            |(mut wi, mut maker_sum, mut taker_sum), (w, maker, taker)| {
                // apply to sum
                wi += &w;
                maker_sum.price += &w * &maker.price;
                taker_sum.price += w * &taker.price;

                maker_sum.exchanges.extend(maker.exchanges);
                taker_sum.exchanges.extend(taker.exchanges);

                // TODO: apply exchange volumes (will do later)
                (wi, maker_sum, taker_sum)
            },
        );

    if total_volume_pct < Rational::ONE || wi == Rational::ZERO {
        return None
    }

    maker.price /= &wi;
    taker.price /= wi;
    Some((maker, taker))
}

// TODO: Potentially collect all sets from 100% to 120% then select best price
fn _closest<'a>(
    iter: impl ParallelIterator<Item = Vec<&'a CexTradePtr<'a>>>,
    vol: &Rational,
) -> Option<Vec<&'a CexTradePtr<'a>>> {
    // sort from lowest to highest volume returning the first
    // does not return a vec that does not have enough volume to fill the arb
    let mut mapped = iter
        .map(|a| (a.iter().map(|t| &t.get().amount).sum::<Rational>(), a))
        .collect::<Vec<_>>();

    mapped.sort_unstable_by(|a, b| a.0.cmp(&b.0));

    mapped
        .into_iter()
        .find_map(|(m_vol, set)| m_vol.ge(vol).then_some(set))
}

struct CexTradePtr<'ptr> {
    raw: *const CexTrades,
    /// used to bound the raw ptr so we can't use it if it goes out of scope.
    _p:  PhantomData<&'ptr u8>,
}

unsafe impl<'a> Send for CexTradePtr<'a> {}
unsafe impl<'a> Sync for CexTradePtr<'a> {}

impl<'ptr> CexTradePtr<'ptr> {
    fn new(raw: &CexTrades) -> Self {
        Self { raw: raw as *const _, _p: PhantomData }
    }

    fn get(&'ptr self) -> &'ptr CexTrades {
        unsafe { &*self.raw }
    }
}

#[derive(Debug)]
pub struct MakerTakerWithVolumeFilled {
    volume_looked_at: Rational,
    prices:           MakerTaker,
}

#[allow(dead_code)]
fn time_weight(t: i64, block_time: u64, lambda_pre: f64, lambda_post: f64) -> f64 {
    let time_difference = (t - block_time as i64).abs() as f64;

    if t < block_time as i64 {
        let exponent = lambda_pre * time_difference;
        E.powf(-exponent)
    } else {
        let exponent = lambda_post * time_difference;
        E.powf(-exponent)
    }
}
