use std::{
    cmp::{max, min},
    f64::consts::E,
    ops::Mul,
};

use alloy_primitives::{Address, FixedBytes};
use itertools::Itertools;
use malachite::{
    num::basic::traits::{One, Zero},
    Rational,
};
use tracing::trace;

use super::{
    config::CexDexTradeConfig,
    utils::{log_insufficient_trade_volume, log_missing_trade_data, PairTradeWalker},
    CexTrades,
};
use crate::{
    constants::{USDC_ADDRESS, USDT_ADDRESS},
    db::cex::CexExchange,
    display::utils::format_etherscan_url,
    normalized_actions::NormalizedSwap,
    pair::Pair,
    FastHashMap, FastHashSet,
};

const PRE_DECAY: f64 = -0.0000005;
const POST_DECAY: f64 = -0.0000002;

const START_POST_TIME_US: u64 = 50_000;
const START_PRE_TIME_US: u64 = 50_000;

const PRE_SCALING_DIFF: u64 = 300_000;
const TIME_STEP: u64 = 10_000;

pub type PriceWithVolume = (Rational, Rational);
pub type MakerTakerWindowVWAP = (WindowExchangePrice, WindowExchangePrice);

#[derive(Debug, Clone, Default)]
pub struct ExchangePath {
    pub price:            Rational,
    pub volume:           Rational,
    // window results
    pub final_start_time: u64,
    pub final_end_time:   u64,
}

#[derive(Debug, Clone, Default)]
pub struct WindowExchangePrice {
    /// the price for this exchange with the volume
    pub exchange_price_with_volume_direct: FastHashMap<CexExchange, ExchangePath>,
    /// the pairs that were traded through in order to get this price.
    /// in the case of a intermediary, this will be 2, otherwise, 1
    pub pairs: Vec<Pair>,
    /// weighted combined price.
    pub global_exchange_price: Rational,
}

impl Mul for WindowExchangePrice {
    type Output = WindowExchangePrice;

    fn mul(mut self, mut rhs: Self) -> Self::Output {
        self.exchange_price_with_volume_direct = self
            .exchange_price_with_volume_direct
            .into_iter()
            .filter_map(|(exchange, mut first_leg)| {
                let second_leg = rhs.exchange_price_with_volume_direct.remove(&exchange)?;
                first_leg.price *= second_leg.price;

                first_leg.final_start_time =
                    min(first_leg.final_start_time, second_leg.final_start_time);

                first_leg.final_end_time = max(first_leg.final_end_time, second_leg.final_end_time);

                Some((exchange, first_leg))
            })
            .collect();

        self.pairs.extend(rhs.pairs);
        self.global_exchange_price *= rhs.global_exchange_price;

        self
    }
}

// trades sorted by time-stamp with the index to block time-stamp closest to the
// block_number
pub struct TimeWindowTrades<'a>(
    pub FastHashMap<&'a CexExchange, FastHashMap<&'a Pair, (usize, &'a Vec<CexTrades>)>>,
);

impl<'a> TimeWindowTrades<'a> {
    //TODO: To optimize we could only calculate the partition point lazily
    pub(crate) fn new_from_cex_trade_map(
        trade_map: &'a mut FastHashMap<CexExchange, FastHashMap<Pair, Vec<CexTrades>>>,
        block_timestamp: u64,
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
                            // Filter out pairs that couldn't be used as intermediaries
                            if !(pair.0 == ex_pair.0
                                || pair.0 == ex_pair.1
                                || pair.1 == ex_pair.0
                                || pair.1 == ex_pair.1)
                            {
                                return None
                            }

                            // Sorts trades by timestamp &
                            trades.sort_unstable_by_key(|k| k.timestamp);

                            let idx =
                                trades.partition_point(|trades| trades.timestamp < block_timestamp);
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
        config: CexDexTradeConfig,
        exchanges: &[CexExchange],
        pair: Pair,
        volume: &Rational,
        timestamp: u64,
        bypass_vol: bool,
        dex_swap: &NormalizedSwap,
        tx_hash: FixedBytes<32>,
    ) -> Option<MakerTakerWindowVWAP> {
        if pair.0 == pair.1 {
            return Some((
                WindowExchangePrice { global_exchange_price: Rational::ONE, ..Default::default() },
                WindowExchangePrice { global_exchange_price: Rational::ONE, ..Default::default() },
            ))
        }

        let res = self
            .get_vwap_price(
                config, exchanges, pair, volume, timestamp, bypass_vol, dex_swap, tx_hash,
            )
            .or_else(|| {
                self.get_vwap_price_via_intermediary(
                    config, exchanges, &pair, volume, timestamp, bypass_vol, dex_swap, tx_hash,
                )
            });

        if res.is_none() {
            tracing::debug!(target: "brontes_types::db::cex::time_window_vwam", ?pair, "No price VMAP found for {}-{} in time window.\n Tx: {}", dex_swap.token_in.symbol, dex_swap.token_out.symbol, format_etherscan_url(&tx_hash));
        }

        res
    }

    fn get_vwap_price_via_intermediary(
        &self,
        config: CexDexTradeConfig,
        exchanges: &[CexExchange],
        pair: &Pair,
        volume: &Rational,
        block_timestamp: u64,
        bypass_vol: bool,
        dex_swap: &NormalizedSwap,
        tx_hash: FixedBytes<32>,
    ) -> Option<MakerTakerWindowVWAP> {
        self.calculate_intermediary_addresses(exchanges, pair)
            .into_iter()
            .filter_map(|intermediary| {
                trace!(target: "brontes_types::db::cex::time_window_vwam", ?intermediary, "trying intermediary");

                let pair0 = Pair(pair.0, intermediary);
                let pair1 = Pair(intermediary, pair.1);

                let mut has_pair0 = false;
                let mut has_pair1 = false;

                for (_, trades) in self.0.iter().filter(|(ex, _)| exchanges.contains(ex)) {
                    has_pair0 |= trades.contains_key(&pair0) || trades.contains_key(&pair0.flip());
                    has_pair1 |= trades.contains_key(&pair1) || trades.contains_key(&pair1.flip());

                    if has_pair1 && has_pair0 {
                        break
                    }
                }

                if !(has_pair0 && has_pair1) {
                    return None
                }

                let mut bypass_intermediary_vol = false;

                // bypass volume requirements for stable pairs
                if pair0.0 == USDC_ADDRESS && pair0.1 == USDT_ADDRESS
                || pair0.0 == USDT_ADDRESS && pair0.1 == USDC_ADDRESS {
                    bypass_intermediary_vol = true;
                }

                tracing::debug!(target: "brontes_types::db::cex::time_window_vwam", ?pair, ?intermediary, ?volume, "trying via intermediary");
                let first_leg = self.get_vwap_price(
                    config,
                    exchanges,
                    pair0,
                    volume,
                    block_timestamp,
                    bypass_vol || bypass_intermediary_vol,
                    dex_swap,
                    tx_hash,
                )?;

                // Volume of second leg
                let second_leg_volume = &first_leg.1.global_exchange_price * volume;

                if pair1.0 == USDT_ADDRESS && pair1.1 == USDC_ADDRESS
                || pair1.0 == USDC_ADDRESS && pair1.1 == USDT_ADDRESS{
                    bypass_intermediary_vol = true;
                }

                let second_leg = self.get_vwap_price(
                    config,
                    exchanges,
                    pair1,
                    &second_leg_volume,
                    block_timestamp,
                    bypass_vol || bypass_intermediary_vol,
                    dex_swap,
                    tx_hash,
                )?;

                let maker = first_leg.0 * second_leg.0;
                let taker = first_leg.1 * second_leg.1;


                Some((maker, taker))
            })
            .max_by_key(|a| a.0.global_exchange_price.clone())
    }

    #[allow(clippy::type_complexity)]
    /// Calculates the Volume Weighted Markout over a dynamic time window.
    ///
    /// This function adjusts the time window dynamically around a given block
    /// time to achieve a sufficient volume of trades for analysis. The
    /// initial time window is set to [-0.5, +2] (relative to
    /// the block time). If the volume is deemed insufficient within this
    /// window, the function extends the post-block window by increments of
    /// 0.1 up to +3. If still insufficient, it then extends the
    /// pre-block window by increments of 0.1 up to -2, while also allowing the
    /// post-block window to increment up to +4. If the volume remains
    /// insufficient, the post-block window may be extended further up to
    /// +5, and the pre-block window to -3.

    /// ## Execution Risk
    /// - **Risk of Price Movements**: Extending the time window increases the
    ///   risk of significant market condition changes that could negatively
    ///   impact arbitrage outcomes.
    ///
    /// ## Bi-Exponential Decay Function
    /// A bi-exponential decay function weights the trades based on their timing
    /// relative to the block time, skewing the weights to favor post-block
    /// trades to account for the certainty in DEX executions. The weight
    /// \(W(t)\) for a trade at time \(t\) is defined as follows:
    ///
    /// If t < BlockTime:  W(t) = exp(-lambda_pre * (BlockTime - t))
    /// If t >= BlockTime: W(t) = exp(-lambda_post * (t - BlockTime))
    ///
    /// Where:
    /// - `t`: timestamp of each trade.
    /// - `BlockTime`: time the block was first seen on the peer-to-peer
    ///   network.
    /// - `lambda_pre` and `lambda_post`: decay rates before and after the block
    ///   time, respectively.
    ///
    /// ## Adjusted Volume Weighted Average Price (VWAP)
    /// The Adjusted VWAP is calculated by integrating both the volume and the
    /// timing weights into the VWAP calculation:
    ///
    /// AdjustedVWAP = (Sum of (Price_i * Volume_i * TimingWeight_i)) / (Sum of
    /// (Volume_i * TimingWeight_i))

    //TODO: This currently expands the time window if the global volume is not met.
    //which means that each exchange is not actually expanded to the point of the
    // full arbitrage volume. We should probably redesign this later on to
    // improve upon this because that feels a bit weird.
    fn get_vwap_price(
        &self,
        config: CexDexTradeConfig,
        exchanges: &[CexExchange],
        pair: Pair,
        vol: &Rational,
        block_timestamp: u64,
        bypass_vol: bool,
        dex_swap: &NormalizedSwap,
        tx_hash: FixedBytes<32>,
    ) -> Option<MakerTakerWindowVWAP> {
        let trade_data = self.get_trades(exchanges, pair, dex_swap, tx_hash)?;

        //TODO: Extend time window if vol is cleared by 1.5

        let mut walker = PairTradeWalker::new(
            trade_data.trades,
            trade_data.indices,
            block_timestamp - START_PRE_TIME_US,
            block_timestamp + START_POST_TIME_US,
        );

        let mut trade_volume_global = Rational::ZERO;
        let mut exchange_vxp = FastHashMap::default();

        while trade_volume_global.le(vol) {
            let trades = walker.get_trades_for_window();
            for trade in trades {
                let trade = trade.get();

                // See explanation of trade representation in the book
                let adjusted_trade = trade.adjust_for_direction(trade_data.direction);

                let (m_fee, t_fee) = trade.exchange.fees();
                let weight = calculate_weight(block_timestamp, trade.timestamp);

                let (
                    vxp_maker,
                    vxp_taker,
                    trade_volume_weight,
                    trade_volume_ex,
                    start_time,
                    end_time,
                ) = exchange_vxp.entry(trade.exchange).or_insert((
                    Rational::ZERO,
                    Rational::ZERO,
                    Rational::ZERO,
                    Rational::ZERO,
                    0u64,
                    0u64,
                ));

                *vxp_maker += (&adjusted_trade.price * (Rational::ONE - m_fee))
                    * &adjusted_trade.amount
                    * &weight;
                *vxp_taker += (&adjusted_trade.price * (Rational::ONE - t_fee))
                    * &adjusted_trade.amount
                    * &weight;
                *trade_volume_weight += &adjusted_trade.amount * weight;
                *trade_volume_ex += &adjusted_trade.amount;
                trade_volume_global += &adjusted_trade.amount;

                *start_time = walker.min_timestamp;
                *end_time = walker.max_timestamp;
            }

            if walker.get_min_time_delta(block_timestamp) >= config.time_window_before_us
                || walker.get_max_time_delta(block_timestamp) >= config.time_window_after_us
            {
                break
            }

            let min_expand = (walker.get_max_time_delta(block_timestamp) >= PRE_SCALING_DIFF)
                .then_some(TIME_STEP)
                .unwrap_or_default();

            walker.expand_time_bounds(min_expand, TIME_STEP);
        }

        if &trade_volume_global < vol && !bypass_vol {
            log_insufficient_trade_volume(
                pair,
                dex_swap,
                &tx_hash,
                trade_volume_global,
                vol.clone(),
            );
            return None
        }

        let mut maker = FastHashMap::default();
        let mut taker = FastHashMap::default();

        let mut global_maker = Rational::ZERO;
        let mut global_taker = Rational::ZERO;

        for (ex, (vxp_maker, vxp_taker, trade_vol_weight, trade_vol, start_time, end_time)) in
            exchange_vxp
        {
            if trade_vol_weight == Rational::ZERO {
                continue
            }
            let maker_price = vxp_maker / &trade_vol_weight;
            let taker_price = vxp_taker / &trade_vol_weight;

            global_maker += &maker_price * &trade_vol;
            global_taker += &taker_price * &trade_vol;

            let maker_path = ExchangePath {
                volume:           trade_vol.clone(),
                price:            maker_price,
                final_end_time:   end_time,
                final_start_time: start_time,
            };
            let taker_path = ExchangePath {
                volume:           trade_vol.clone(),
                price:            taker_price,
                final_end_time:   end_time,
                final_start_time: start_time,
            };

            maker.insert(ex, maker_path);
            taker.insert(ex, taker_path);
        }

        if trade_volume_global == Rational::ZERO {
            log_insufficient_trade_volume(
                pair,
                dex_swap,
                &tx_hash,
                trade_volume_global,
                vol.clone(),
            );
            return None
        }

        let global_maker = global_maker / &trade_volume_global;
        let global_taker = global_taker / &trade_volume_global;

        let maker_ret = WindowExchangePrice {
            exchange_price_with_volume_direct: maker,
            global_exchange_price: global_maker,
            pairs: vec![pair],
        };
        let taker_ret = WindowExchangePrice {
            exchange_price_with_volume_direct: taker,
            pairs: vec![pair],
            global_exchange_price: global_taker,
        };

        Some((maker_ret, taker_ret))
    }

    pub fn get_trades(
        &'a self,
        exchanges: &[CexExchange],
        pair: Pair,
        dex_swap: &NormalizedSwap,
        tx_hash: FixedBytes<32>,
    ) -> Option<TradeData<'a>> {
        let (mut indices, mut trades) = self.query_trades(exchanges, &pair);

        if trades.is_empty() {
            let flipped_pair = pair.flip();
            (indices, trades) = self.query_trades(exchanges, &flipped_pair);

            if !trades.is_empty() {
                trace!(
                    target: "brontes_types::db::cex::time_window_vwam",
                    trade_qty = %trades.len(),
                    "have trades (flipped pair)"
                );
                return Some(TradeData { indices, trades, direction: Direction::Buy });
            } else {
                log_missing_trade_data(dex_swap, &tx_hash);
                return None;
            }
        }

        trace!(
            target: "brontes_types::db::cex::time_window_vwam",
            trade_qty = %trades.len(),
            "have trades"
        );
        Some(TradeData { indices, trades, direction: Direction::Sell })
    }

    fn query_trades(
        &'a self,
        exchanges: &[CexExchange],
        pair: &Pair,
    ) -> (FastHashMap<CexExchange, (usize, usize)>, Vec<(CexExchange, &'a Vec<CexTrades>)>) {
        self.0
            .iter()
            .filter(|(e, _)| exchanges.contains(e))
            .filter_map(|(exchange, pairs)| Some((**exchange, pairs.get(pair)?)))
            .map(|(ex, (idx, trades))| ((ex, (idx.saturating_sub(1), *idx)), (ex, *trades)))
            .unzip()
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
                        (trade_pair.0 == pair.0)
                            .then_some(trade_pair.1)
                            .or_else(|| (trade_pair.1 == pair.0).then_some(trade_pair.0))
                            .or_else(|| (trade_pair.1 == pair.1).then_some(trade_pair.0))
                            .or_else(|| (trade_pair.0 == pair.1).then_some(trade_pair.1))
                    })
                    .collect_vec()
            })
            .collect::<FastHashSet<_>>()
    }
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

fn calculate_weight(block_time: u64, trade_time: u64) -> Rational {
    let pre = trade_time < block_time;

    Rational::try_from_float_simplest(if pre {
        E.powf(PRE_DECAY * (block_time - trade_time) as f64)
    } else {
        E.powf(POST_DECAY * (trade_time - block_time) as f64)
    })
    .unwrap()
}

#[derive(Debug, Clone, Copy)]
pub enum Direction {
    Buy,
    Sell,
}

#[derive(Debug)]
pub struct TradeData<'a> {
    pub indices:   FastHashMap<CexExchange, (usize, usize)>,
    pub trades:    Vec<(CexExchange, &'a Vec<CexTrades>)>,
    pub direction: Direction,
}
