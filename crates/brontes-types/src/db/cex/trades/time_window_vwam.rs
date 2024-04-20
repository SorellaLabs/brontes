use std::{f64::consts::E, ops::Mul};

use alloy_primitives::Address;
use itertools::Itertools;
use malachite::{
    num::basic::traits::{One, Zero},
    Rational,
};
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};

use super::{utils::PairTradeWalker, CexTrades};
use crate::{db::cex::CexExchange, pair::Pair, FastHashMap, FastHashSet};

const PRE_DECAY: f64 = -0.5;
const POST_DECAY: f64 = -0.2;

const START_POST_TIME_US: u64 = 2_000_000;
const START_PRE_TIME_US: u64 = 500_000;

const MAX_POST_TIME_US: u64 = 5_000_000;
const MAX_PRE_TIME_US: u64 = 3_000_000;

const PRE_SCALING_DIFF: u64 = 3_000_000;
const TIME_STEP: u64 = 100_000;

pub type PriceWithVolume = (Rational, Rational);
pub type MakerTakerWindowVwam = (WindowExchangePrice, WindowExchangePrice);

#[derive(Debug, Clone, Default)]
pub struct WindowExchangePrice {
    /// the price for this exchange with the volume
    pub exchange_price_with_volume_direct: FastHashMap<CexExchange, PriceWithVolume>,
    /// weighted combined price.
    pub global_exchange_price:             Rational,
}

// used for intermediary calcs
impl Mul for WindowExchangePrice {
    type Output = WindowExchangePrice;

    fn mul(mut self, mut rhs: Self) -> Self::Output {
        // adjust the price with volume
        self.exchange_price_with_volume_direct = self
            .exchange_price_with_volume_direct
            .into_iter()
            .filter_map(|(exchange, (this_price, this_vol))| {
                let (other_price, other_vol) =
                    rhs.exchange_price_with_volume_direct.remove(&exchange)?;

                let this_vol = &this_price * &this_vol;
                let other_vol = &other_vol * &other_price;
                let vol = this_vol + other_vol;

                let price = this_price * other_price;

                Some((exchange, (price, vol)))
            })
            .collect();

        self.global_exchange_price *= rhs.global_exchange_price;

        self
    }
}

// trades sorted by time-stamp with the index to block time-stamp closest to the
// block_number
pub struct TimeWindowTrades<'a>(
    FastHashMap<&'a CexExchange, FastHashMap<&'a Pair, (usize, &'a Vec<CexTrades>)>>,
);

impl<'a> TimeWindowTrades<'a> {
    pub(crate) fn new_from_cex_trade_map(
        trade_map: &'a mut FastHashMap<CexExchange, FastHashMap<Pair, Vec<CexTrades>>>,
        timestamp: u64,
        exchanges: &'a [CexExchange],
        pair: Pair,
    ) -> Self {
        let map = trade_map
            .iter_mut()
            .filter_map(|(ex, pairs)| {
                if !exchanges.contains(ex) || pair.0 == pair.1 {
                    return None
                }

                Some((
                    ex,
                    pairs
                        .iter_mut()
                        .filter_map(|(ex_pair, trades)| {
                            if !(pair.0 == ex_pair.0
                                || pair.0 == ex_pair.1
                                || pair.1 == ex_pair.0
                                || pair.1 == ex_pair.1)
                            {
                                return None
                            }
                            // because we know that for this, we will only be
                            // touching pairs that are in
                            trades.sort_unstable_by_key(|k| k.timestamp);
                            let idx = trades.partition_point(|trades| trades.timestamp < timestamp);
                            Some((ex_pair, (idx, &*trades)))
                        })
                        .collect(),
                ))
            })
            .collect::<FastHashMap<&CexExchange, FastHashMap<&Pair, (usize, &Vec<CexTrades>)>>>();

        Self(map)
    }

    pub(crate) fn get_price(
        &self,
        exchanges: &[CexExchange],
        pair: Pair,
        volume: &Rational,
        timestamp: u64,
    ) -> Option<MakerTakerWindowVwam> {
        if pair.0 == pair.1 {
            return Some((
                WindowExchangePrice { global_exchange_price: Rational::ONE, ..Default::default() },
                WindowExchangePrice { global_exchange_price: Rational::ONE, ..Default::default() },
            ))
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
        timestamp: u64,
    ) -> Option<MakerTakerWindowVwam> {
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

                let new_vol = volume * &res.0.global_exchange_price;
                let pair1_v = self.get_vwam_price(exchanges, pair1, &new_vol, timestamp)?;

                let maker = res.0 * pair1_v.0;
                let taker = res.1 * pair1_v.1;

                Some((maker, taker))
            })
            .max_by_key(|a| a.0.global_exchange_price.clone())
    }

    fn get_vwam_price(
        &self,
        exchanges: &[CexExchange],
        pair: Pair,
        vol: &Rational,
        timestamp: u64,
    ) -> Option<MakerTakerWindowVwam> {
        let (ptrs, trades): (FastHashMap<CexExchange, (usize, usize)>, Vec<(CexExchange, _)>) =
            self.0
                .iter()
                .filter(|(e, _)| exchanges.contains(e))
                .filter_map(|(exchange, trades)| Some((**exchange, trades.get(&pair)?)))
                .map(|(ex, (idx, trades))| ((ex, (idx + 1, *idx)), (ex, *trades)))
                .unzip();

        let mut walker = PairTradeWalker::new(
            trades,
            ptrs,
            timestamp - START_PRE_TIME_US,
            timestamp + START_POST_TIME_US,
        );

        let mut trade_volume_global = Rational::ZERO;
        let mut exchange_vxp = FastHashMap::default();

        while trade_volume_global.le(vol) {
            for trade in walker.get_trades_for_window() {
                let trade = trade.get();
                let (m_fee, t_fee) = trade.exchange.fees();
                let weight = calcuate_weight(timestamp as u64, trade.timestamp as u64);

                let (vxp_maker, vxp_taker, trade_volume_weight, trade_volume_ex) = exchange_vxp
                    .entry(trade.exchange)
                    .or_insert((Rational::ZERO, Rational::ZERO, Rational::ZERO, Rational::ZERO));

                *vxp_maker += (&trade.price * (Rational::ONE - m_fee)) * &trade.amount * &weight;
                *vxp_taker += (&trade.price * (Rational::ONE - t_fee)) * &trade.amount * &weight;
                *trade_volume_weight += &trade.amount * weight;
                *trade_volume_ex += &trade.amount;

                trade_volume_global += &trade.amount;
            }

            if walker.get_min_time_delta(timestamp) >= MAX_PRE_TIME_US
                || walker.get_max_time_delta(timestamp) >= MAX_POST_TIME_US
            {
                break
            }

            let min_expand = (walker.get_max_time_delta(timestamp) >= PRE_SCALING_DIFF)
                .then_some(TIME_STEP)
                .unwrap_or_default();

            walker.expand_time_bounds(min_expand, TIME_STEP);
        }

        if &trade_volume_global < vol {
            return None
        }

        let mut maker = FastHashMap::default();
        let mut taker = FastHashMap::default();

        let mut global_maker = Rational::ZERO;
        let mut global_taker = Rational::ZERO;

        for (ex, (vxp_maker, vxp_taker, trade_vol_weight, trade_vol)) in exchange_vxp {
            let maker_price = vxp_maker / &trade_vol_weight;
            let taker_price = vxp_taker / &trade_vol_weight;

            global_maker += &maker_price * &trade_vol;
            global_taker += &taker_price * &trade_vol;

            maker.insert(ex, (maker_price, trade_vol.clone()));
            taker.insert(ex, (taker_price, trade_vol));
        }

        let global_maker = global_maker / &trade_volume_global;
        let global_taker = global_taker / &trade_volume_global;

        let maker_ret = WindowExchangePrice {
            exchange_price_with_volume_direct: maker,
            global_exchange_price:             global_maker,
        };
        let taker_ret = WindowExchangePrice {
            exchange_price_with_volume_direct: taker,
            global_exchange_price:             global_taker,
        };

        Some((maker_ret, taker_ret))
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
