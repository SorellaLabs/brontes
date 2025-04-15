use std::{f64::consts::E, marker::PhantomData};

use ahash::HashSetExt;
use alloy_primitives::{Address, TxHash};
use malachite::{num::basic::traits::Zero, Rational};
use tracing::trace;

use crate::{db::cex::trades::BASE_EXECUTION_QUALITY, FastHashSet};
const TIME_BASKET_SIZE: u64 = 100_000;

use super::{optimistic::OptimisticTradeData, CexDexTradeConfig, CexTrades};
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
        let mut trade_res: Vec<CexTradePtr<'a>> = Vec::with_capacity(1000);

        for (exchange, trades) in &self.trades {
            let Some((lower_idx, upper_idx)) = self.exchange_ptrs.get_mut(exchange) else {
                continue
            };

            // Gets trades before the block timestamp that are within the current pre block
            // time window
            if *lower_idx > 0 {
                loop {
                    let next_trade = &trades[*lower_idx];
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
            let max = trades.len() - 1;
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

pub struct CexTradePtr<'ptr> {
    raw: *const CexTrades,
    /// used to bound the raw ptr so we can't use it if it goes out of scope.
    _p:  PhantomData<&'ptr u8>,
}

pub struct TradeBasket<'a> {
    trade_index: usize,
    trades:      Vec<CexTradePtr<'a>>,
    pub volume:  Rational,
}

impl<'a> TradeBasket<'a> {
    pub fn new(mut trades: Vec<CexTradePtr<'a>>, quality_pct: usize, volume: Rational) -> Self {
        let length = trades.len() - 1;
        let trade_index = length - (length * quality_pct / 100);
        trades.sort_unstable_by_key(|k| k.get().price.clone());

        Self { trade_index, trades, volume }
    }

    pub fn get_trades_used(&self, volume_to_fill: &Rational) -> (Vec<CexTrades>, Rational) {
        let mut trades_used = Vec::new();
        let mut remaining_volume = volume_to_fill.clone();

        for trade in self
            .trades
            .iter()
            .skip((self.trades.len() - 1) - self.trade_index)
        {
            let trade_data = trade.get();

            if trade_data.amount >= remaining_volume {
                let mut final_trade = trade_data.clone();
                final_trade.amount = remaining_volume;
                trades_used.push(final_trade);
                remaining_volume = Rational::ZERO;
                break
            } else {
                trades_used.push(trade_data.clone());
                remaining_volume -= &trade_data.amount;
            }

            if remaining_volume == Rational::ZERO {
                break
            }
        }

        (trades_used, remaining_volume)
    }
}

pub struct SortedTrades<'a>(pub FastHashMap<&'a Pair, ((usize, usize), Vec<&'a CexTrades>)>);

impl<'a> SortedTrades<'a> {
    pub fn new_from_cex_trade_map(
        trade_map: &'a FastHashMap<CexExchange, FastHashMap<Pair, Vec<CexTrades>>>,
        exchanges: &[CexExchange],
        pair: Pair,
        block_timestamp: u64,
    ) -> Self {
        let mut consolidated_trades: FastHashMap<&'a Pair, Vec<&'a CexTrades>> =
            FastHashMap::default();

        for (ex, pairs) in trade_map.iter() {
            if !exchanges.contains(ex) || pair.0 == pair.1 {
                continue
            }

            for (ex_pair, trades) in pairs.iter() {
                // Filter out pairs that couldn't be used as intermediaries
                if !(pair.0 == ex_pair.0
                    || pair.0 == ex_pair.1
                    || pair.1 == ex_pair.0
                    || pair.1 == ex_pair.1)
                {
                    continue
                }

                consolidated_trades
                    .entry(ex_pair)
                    .or_default()
                    .extend(trades.iter());
            }
        }

        let pair_trades = consolidated_trades
            .into_iter()
            .map(|(pair, trades)| {
                let partition_point = trades.partition_point(|t| t.timestamp < block_timestamp);
                let lower_index = if partition_point > 0 { partition_point - 1 } else { 0 };
                let upper_index = partition_point;

                (pair, ((lower_index, upper_index), trades))
            })
            .collect();

        Self(pair_trades)
    }

    // Returns the intermediary addresses, assuming a single hop
    pub fn calculate_intermediary_addresses(&self, pair: &Pair) -> FastHashSet<Address> {
        let (token_a, token_b) = (pair.0, pair.1);
        let mut connected_to_a = FastHashSet::new();
        let mut connected_to_b = FastHashSet::new();

        self.0.keys().for_each(|trade_pair| {
            if trade_pair.0 == token_a {
                connected_to_a.insert(trade_pair.1);
            } else if trade_pair.1 == token_a {
                connected_to_a.insert(trade_pair.0);
            }

            if trade_pair.0 == token_b {
                connected_to_b.insert(trade_pair.1);
            } else if trade_pair.1 == token_b {
                connected_to_b.insert(trade_pair.0);
            }
        });

        connected_to_a
            .intersection(&connected_to_b)
            .cloned()
            .collect()
    }
}

pub struct TimeBasketQueue<'a> {
    pub baskets:       Vec<TradeBasket<'a>>,
    min_timestamp:     u64,
    max_timestamp:     u64,
    current_pre_time:  u64,
    current_post_time: u64,
    pub volume:        Rational,
    quality_pct:       Option<FastHashMap<CexExchange, usize>>,
    indexes:           (usize, usize),
    trades:            Vec<CexTrades>,
}

impl TimeBasketQueue<'_> {
    pub(crate) fn new(
        trade_data: OptimisticTradeData,
        block_timestamp: u64,
        quality: Option<FastHashMap<CexExchange, usize>>,
        config: &CexDexTradeConfig,
    ) -> Self {
        Self {
            current_pre_time:  block_timestamp,
            current_post_time: block_timestamp,
            min_timestamp:     block_timestamp - config.initial_optimistic_pre_block_us,
            max_timestamp:     block_timestamp + config.initial_optimistic_post_block_us,
            indexes:           trade_data.indices,
            trades:            trade_data.trades,
            quality_pct:       quality,
            volume:            Rational::ZERO,
            baskets:           Vec::with_capacity(20),
        }
    }

    pub fn construct_time_baskets(&mut self) {
        self.construct_forward_baskets();
        self.construct_backward_baskets();
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

        self.construct_time_baskets();
    }

    fn construct_forward_baskets(&mut self) {
        while self.current_post_time < self.max_timestamp && self.indexes.1 + 1 < self.trades.len()
        {
            self.current_post_time += TIME_BASKET_SIZE;

            // Adjust the last basket to cover remaining time
            if self.current_post_time > self.max_timestamp {
                self.current_post_time = self.max_timestamp;
            }

            let mut basket_trades = Vec::new();
            let mut basket_volume = Rational::ZERO;

            while self.indexes.1 < self.trades.len() {
                let trade = &self.trades[self.indexes.1];
                if trade.timestamp > self.current_post_time {
                    break
                }
                basket_trades.push(CexTradePtr::new(trade));
                basket_volume += &trade.amount;
                self.indexes.1 += 1;
            }

            if !basket_trades.is_empty() {
                self.volume += &basket_volume;
                let basket = TradeBasket::new(
                    basket_trades,
                    self.quality_pct
                        .as_ref()
                        .map(|map| {
                            *map.get(&CexExchange::Binance)
                                .unwrap_or(&BASE_EXECUTION_QUALITY)
                        })
                        .unwrap_or(BASE_EXECUTION_QUALITY),
                    basket_volume,
                );
                self.baskets.push(basket);
            }

            // Break if we've reached the max timestamp
            if self.current_post_time >= self.max_timestamp {
                break
            }
        }
    }

    fn construct_backward_baskets(&mut self) {
        while self.current_pre_time > self.min_timestamp && self.indexes.0 > 0 {
            self.current_pre_time -= TIME_BASKET_SIZE;

            // Adjust the last basket to cover remaining time
            if self.current_pre_time < self.min_timestamp {
                self.current_pre_time = self.min_timestamp;
            }

            let mut basket_trades = Vec::new();
            let mut basket_volume = Rational::ZERO;

            while self.indexes.0 > 0 {
                let trade = &self.trades[self.indexes.0];
                if trade.timestamp < self.current_pre_time {
                    break
                }
                basket_trades.push(CexTradePtr::new(trade));
                basket_volume += &trade.amount;
                self.indexes.0 -= 1;
            }

            if !basket_trades.is_empty() {
                self.volume += &basket_volume;
                let basket = TradeBasket::new(
                    basket_trades,
                    self.quality_pct
                        .as_ref()
                        .map(|map| {
                            *map.get(&CexExchange::Binance)
                                .unwrap_or(&BASE_EXECUTION_QUALITY)
                        })
                        .unwrap_or(BASE_EXECUTION_QUALITY),
                    basket_volume,
                );
                self.baskets.push(basket);
            }

            // Break if we've reached the min timestamp
            if self.current_pre_time <= self.min_timestamp {
                break
            }
        }
    }
}

unsafe impl Send for CexTradePtr<'_> {}
unsafe impl Sync for CexTradePtr<'_> {}

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

/// Calculates the weight for a trade using a bi-exponential decay function
/// based on its timestamp relative to a block time.
///
/// This function is designed to account for the risk associated with the timing
/// of trades in relation to block times in the context of cex-dex
/// arbitrage. This assumption underpins our pricing model: trades that
/// occur further from the block time are presumed to carry higher uncertainty
/// and an increased risk of adverse market conditions potentially impacting
/// arbitrage outcomes. Accordingly, the decay rates (`PRE_DECAY` for pre-block
/// and `POST_DECAY` for post-block) adjust the weight assigned to each trade
/// based on its temporal proximity to the block time.
///
/// Trades after the block are assumed to be generally preferred by arbitrageurs
/// as they have confirmation that their DEX swap is executed. However, this
/// preference can vary for less competitive pairs where the opportunity and
/// timing of execution might differ.
///
/// # Parameters
/// - `block_time`: The timestamp of the block as seen first on the peer-to-peer
///   network.
/// - `trade_time`: The timestamp of the trade to be weighted.
///
/// # Returns
/// Returns a `Rational` representing the calculated weight for the trade. The
/// weight is determined by:
/// - `exp(-PRE_DECAY * (block_time - trade_time))` for trades before the block
///   time.
/// - `exp(-POST_DECAY * (trade_time - block_time))` for trades after the block
///   time.
pub fn calculate_weight(
    block_time: u64,
    trade_time: u64,
    pre_decay: f64,
    post_decay: f64,
) -> Rational {
    let pre = trade_time < block_time;

    Rational::try_from_float_simplest(if pre {
        E.powf(pre_decay * (block_time - trade_time) as f64)
    } else {
        E.powf(post_decay * (trade_time - block_time) as f64)
    })
    .unwrap()
}
