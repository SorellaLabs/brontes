use std::marker::PhantomData;

use alloy_primitives::TxHash;
use malachite::Rational;
use tracing::trace;

use super::CexTrades;
use crate::{
    db::cex::CexExchange, normalized_actions::NormalizedSwap, pair::Pair, utils::ToFloatNearest,
    FastHashMap,
};

/// Manages the traversal and collection of trade data within dynamically
/// adjustable time windows.
///
/// `PairTradeWalker` is initialized with a set of trades and a time window that
/// can be expanded based on trading volume. It uses `exchange_ptrs`
/// to manage the current position within trade lists for each
/// exchange.
///
/// # Fields
/// - `min_timestamp`: Lower bound of the time window (in microseconds).
/// - `max_timestamp`: Upper bound of the time window (in microseconds).
/// - `exchange_ptrs`: Hash map storing pointers to the current trade indices
///   for each exchange. The lower index points to the last trade before the
///   block timestamp (i.e., the most recent trade just before the block time),
///   and the upper index points to the first trade after the block timestamp
///   (i.e., the earliest trade just after the block time).
/// - `trades`: Vector of tuples associating each exchange with a reference to
///   its trade data.

pub struct PairTradeWalker<'a> {
    pub min_timestamp: u64,
    pub max_timestamp: u64,
    exchange_ptrs:     FastHashMap<CexExchange, (usize, usize)>,
    trades:            Vec<(CexExchange, &'a Vec<CexTrades>)>,
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

    /// Retrieves trades within the specified time window for each exchange.
    ///
    /// Iterates over trades for each exchange listed in `trades`, adjusting
    /// `exchange_ptrs` to include only trades within the current bounds
    /// defined by `min_timestamp` and `max_timestamp`.
    ///
    /// # Returns
    /// A vector of `CexTradePtr` pointing to the trades that meet the time
    /// window criteria.

    pub(crate) fn get_trades_for_window(&mut self) -> Vec<CexTradePtr<'a>> {
        let mut trade_res: Vec<CexTradePtr<'a>> = Vec::with_capacity(420);

        for (exchange, trades) in &self.trades {
            let Some((lower_idx, upper_idx)) = self.exchange_ptrs.get_mut(exchange) else {
                continue
            };

            // Gets trades before the block timestamp that are within the current pre block
            // time window
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

            // Gets trades after the block timestamp that are within the current post block
            // time window
            let max = trades.len();
            if *upper_idx < max {
                loop {
                    let next_trade = &trades[*upper_idx];
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

/// It's ok that we create 2 of these for pair price and intermediary price
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

pub fn log_missing_trade_data(dex_swap: &NormalizedSwap, tx_hash: &TxHash) {
    trace!(
        target: "brontes::time_window_vwam::missing_trade_data",
        "\n\x1b[1;No trade data for {} - {}:\x1b[0m\n\
         - Token Contracts:\n\
            * Token Out: https://etherscan.io/address/{}\n\
            * Token In: https://etherscan.io/address/{}\n\
         - Transaction Hash: https://etherscan.io/tx/{}",
        dex_swap.token_out_symbol(),
        dex_swap.token_in_symbol(),
        dex_swap.token_out.address,
        dex_swap.token_in.address,
        tx_hash
    );
}

pub fn log_insufficient_trade_volume(
    pair: Pair,
    dex_swap: &NormalizedSwap,
    tx_hash: &TxHash,
    trade_volume_global: Rational,
    required_volume: Rational,
) {
    trace!(
        target: "brontes::time_window_vwam::insufficient_trade_volume",
        "\n\x1b[1;31mInsufficient Trade Volume for {} - {}:\x1b[0m\n\
         - Cex Volume:  {:.6}\n\
         - Required Volume:  {:.6}\n\
         - Token Contracts:\n\
            * Token Out: https://etherscan.io/address/{}\n\
            * Token In: https://etherscan.io/address/{}\n\
         - Transaction Hash: https://etherscan.io/tx/{}\n\
        - pair {:#?}
                                   ",
        dex_swap.token_out_symbol(),
        dex_swap.token_in_symbol(),
        trade_volume_global.to_float(),
        required_volume.to_float(),
        dex_swap.token_out.address,
        dex_swap.token_in.address,
        tx_hash,
        pair
    );
}
