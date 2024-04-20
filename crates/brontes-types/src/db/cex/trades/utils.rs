use std::marker::PhantomData;

use super::CexTrades;
use crate::{db::cex::CexExchange, FastHashMap};

/// used for time window vwam
pub struct PairTradeWalker<'a> {
    min_timestamp: u64,
    max_timestamp: u64,
    exchange_ptrs: FastHashMap<CexExchange, (usize, usize)>,
    trades:        Vec<(CexExchange, &'a Vec<CexTrades>)>,
}

impl<'a> PairTradeWalker<'a> {
    pub fn new(
        trades: Vec<(CexExchange, &'a Vec<CexTrades>)>,
        exchange_ptrs: FastHashMap<CexExchange, (usize, usize)>,
        min_timestamp: u64,
        max_timestamp: u64,
    ) -> Self {
        Self { max_timestamp, min_timestamp, trades, exchange_ptrs }
    }

    pub fn get_min_time_delta(&self, timestamp: u64) -> u64 {
        timestamp - self.min_timestamp
    }

    pub fn get_max_time_delta(&self, timestamp: u64) -> u64 {
        self.max_timestamp - timestamp
    }

    pub fn expand_time_bounds(&mut self, min: u64, max: u64) {
        self.min_timestamp -= min;
        self.max_timestamp += max;
    }

    pub(crate) fn get_trades_for_window(&mut self) -> Vec<CexTradePtr<'a>> {
        let mut trade_res: Vec<CexTradePtr<'a>> = Vec::with_capacity(420);

        for (exchange, trades) in &self.trades {
            let Some((lower_idx, upper_idx)) = self.exchange_ptrs.get_mut(exchange) else {
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

    pub(crate) fn next_best_trade(&mut self) -> Option<CexTradePtr<'a>> {
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

pub(crate) struct CexTradePtr<'ptr> {
    raw: *const CexTrades,
    /// used to bound the raw ptr so we can't use it if it goes out of scope.
    _p:  PhantomData<&'ptr u8>,
}

unsafe impl<'a> Send for CexTradePtr<'a> {}
unsafe impl<'a> Sync for CexTradePtr<'a> {}

impl<'ptr> CexTradePtr<'ptr> {
    pub(crate) fn new(raw: &CexTrades) -> Self {
        Self { raw: raw as *const _, _p: PhantomData }
    }

    pub(crate) fn get(&'ptr self) -> &'ptr CexTrades {
        unsafe { &*self.raw }
    }
}
