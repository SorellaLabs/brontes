use std::{cmp::max, fmt::Display};

use alloy_primitives::{Address, FixedBytes};
use itertools::Itertools;
use malachite::{
    num::basic::traits::{One, Zero},
    Rational,
};

use super::{
    cex_trades::CexTradeMap,
    utils::{CexTradePtr, PairTradeQueue},
};
use crate::{
    db::cex::{utils::log_missing_trade_data, CexExchange},
    normalized_actions::NormalizedSwap,
    pair::Pair,
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

pub type MakerTaker = (ExchangePrice, ExchangePrice);

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
    pub(crate) fn get_price(
        &mut self,
        exchanges: &[CexExchange],
        pair: &Pair,
        volume: &Rational,
        quality: Option<FastHashMap<CexExchange, FastHashMap<Pair, usize>>>,
        bypass_vol: bool,
        dex_swap: &NormalizedSwap,
        tx_hash: FixedBytes<32>,
    ) -> Option<MakerTaker> {
        if pair.0 == pair.1 {
            return Some((
                ExchangePrice { exchanges: vec![], price: Rational::ONE },
                ExchangePrice { exchanges: vec![], price: Rational::ONE },
            ))
        }

        // order the possible trade pairs.
        self.ensure_proper_order(exchanges, pair);

        let res = self
            .get_vwam_no_intermediary(
                exchanges,
                pair,
                volume,
                quality.as_ref(),
                bypass_vol,
                dex_swap,
                tx_hash,
            )
            .or_else(|| {
                self.get_vwam_via_intermediary(
                    exchanges,
                    pair,
                    volume,
                    quality.as_ref(),
                    dex_swap,
                    tx_hash,
                )
            });

        if res.is_none() {
            tracing::debug!(?pair, "no vwam found");
        }

        res
    }

    fn ensure_proper_order(&mut self, exchanges: &[CexExchange], pair: &Pair) {
        self.0.iter_mut().for_each(|(ex, pairs)| {
            if !exchanges.contains(ex) || pair.0 == pair.1 {
                return
            }
            pairs.iter_mut().for_each(|(ex_pair, trades)| {
                if !(pair.0 == ex_pair.0
                    || pair.0 == ex_pair.1
                    || pair.1 == ex_pair.0
                    || pair.1 == ex_pair.1)
                {
                    return
                }
                trades.sort_unstable_by_key(|k| k.price.clone());
            });
        })
    }

    fn calculate_intermediary_addresses(
        &self,
        exchanges: &[CexExchange],
        pair: &Pair,
    ) -> FastHashSet<Address> {
        self.0
            .iter()
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
        dex_swap: &NormalizedSwap,
        tx_hash: FixedBytes<32>,
    ) -> Option<MakerTaker> {
        let (pair0_vwams, pair1_vwams) = self
            .calculate_intermediary_addresses(exchanges, pair)
            .into_iter()
            .filter_map(|intermediary| {
                // usdc / bnb 0.004668534080298786price
                let pair0 = Pair(pair.1, intermediary);
                // bnb / eth 0.1298price
                let pair1 = Pair(intermediary, pair.0);
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
                    self.get_vwam_via_intermediary_spread(
                        exchanges, &pair0, volume, quality, dex_swap, tx_hash,
                    )?,
                );

                let new_vol = volume * &res.prices.0.price;

                Some((
                    (i, res),
                    (
                        intermediary,
                        self.get_vwam_via_intermediary_spread(
                            exchanges, &pair1, &new_vol, quality, dex_swap, tx_hash,
                        )?,
                    ),
                ))
            })
            .fold(
                (FastHashMap::default(), FastHashMap::default()),
                |(mut pair0_vwam, mut pair1_vwam), ((iter0, prices0), (iter1, prices1))| {
                    pair0_vwam.entry(iter0).or_insert(vec![]).push(prices0);
                    pair1_vwam.entry(iter1).or_insert(vec![]).push(prices1);
                    (pair0_vwam, pair1_vwam)
                },
            );

        // calculates best possible mult cross pair price
        calculate_multi_cross_pair(pair0_vwams, pair1_vwams, volume)
    }

    pub fn get_vwam_via_intermediary_spread(
        &self,
        exchanges: &[CexExchange],
        pair: &Pair,
        volume: &Rational,
        quality: Option<&FastHashMap<CexExchange, FastHashMap<Pair, usize>>>,
        dex_swap: &NormalizedSwap,
        tx_hash: FixedBytes<32>,
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
            log_missing_trade_data(dex_swap, &tx_hash);
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
        bypass_vol: bool,
        dex_swap: &NormalizedSwap,
        tx_hash: FixedBytes<32>,
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
            log_missing_trade_data(dex_swap, &tx_hash);
            return None
        }
        // Populate trade queue per exchange
        // - This utilizes the quality percent number to set the number of trades that
        //   will be assessed in picking a bucket to calculate the vwam with. A lower
        //   quality percent will cause us to examine more trades (go deeper into the
        //   vec) - resulting in a potentially worse price (remember, trades are sorted
        //   by price)
        let trade_queue = PairTradeQueue::new(trades, quality_pct);

        self.get_most_accurate_basket(trade_queue, volume, bypass_vol)
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
        bypass_vol: bool,
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

        if &cur_vol < volume && !bypass_vol {
            return None
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

        Some((maker, taker))
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
                            price:     &first_vwam.prices.0.price / &second_vwam.prices.0.price,
                        };

                        let taker = ExchangePrice {
                            exchanges: taker_exchanges,
                            price:     &first_vwam.prices.1.price / &second_vwam.prices.1.price,
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
    iter: impl Iterator<Item = Vec<&'a CexTradePtr<'a>>>,
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

#[derive(Debug)]
pub struct MakerTakerWithVolumeFilled {
    volume_looked_at: Rational,
    prices:           MakerTaker,
}
