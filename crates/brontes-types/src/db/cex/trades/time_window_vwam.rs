use core::f64;
use std::f64::consts::E;

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

// trades sorted by time-stamp with the index to block time-stamp closest to the
// block_number
pub struct CexTradeMap2(FastHashMap<CexExchange, FastHashMap<Pair, (usize, Vec<CexTrades>)>>);

impl CexTradeMap2 {
    pub fn get_price(
        &self,
        exchanges: &[CexExchange],
        pair: Pair,
        volume: &Rational,
        timestamp: u64,
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
        timestamp: u64,
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
        timestamp: u64,
    ) -> Option<(Rational, Rational)> {
        let (ptrs, trades): (FastHashMap<CexExchange, (usize, usize)>, Vec<(CexExchange, _)>) =
            self.0
                .iter()
                .filter(|(e, _)| exchanges.contains(e))
                .filter_map(|(exchange, trades)| Some((*exchange, trades.get(&pair)?)))
                .map(|(ex, (idx, trades))| ((ex, (idx + 1, *idx)), (ex, trades)))
                .unzip();

        let mut walker = PairTradeWalker::new(
            trades,
            ptrs,
            timestamp - START_PRE_TIME_US,
            timestamp + START_POST_TIME_US,
        );

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
