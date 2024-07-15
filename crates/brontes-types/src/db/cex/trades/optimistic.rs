use std::{fmt::Display, ops::Mul};

use alloy_primitives::FixedBytes;
use itertools::Itertools;
use malachite::{
    num::basic::traits::{One, Zero},
    Rational,
};

use super::config::CexDexTradeConfig;
use crate::{
    db::cex::{
        time_window_vwam::Direction,
        trades::SortedTrades,
        utils::{log_missing_trade_data, TimeBasketQueue},
        CexExchange, CexTrades,
    },
    display::utils::format_etherscan_url,
    mev::OptimisticTrade,
    normalized_actions::NormalizedSwap,
    pair::Pair,
    utils::ToFloatNearest,
    FastHashMap,
};

pub const BASE_EXECUTION_QUALITY: usize = 70;

const PRE_SCALING_DIFF: u64 = 200_000;
const TIME_STEP: u64 = 100_000;

/// The amount of excess volume a trade can do to be considered
/// as part of execution
const EXCESS_VOLUME_PCT: Rational = Rational::const_from_unsigneds(10, 100);

/// the calculated price based off of trades with the estimated exchanges with
/// volume amount that where used to hedge
#[derive(Debug, Clone)]
pub struct ExchangePrice {
    // cex exchange with amount of volume executed on it
    pub trades_used: Vec<OptimisticTrade>,
    /// the pairs that were traded through in order to get this price.
    /// in the case of a intermediary, this will be 2, otherwise, 1
    pub pairs:       Vec<Pair>,
    pub final_price: Rational,
}

impl Mul for ExchangePrice {
    type Output = ExchangePrice;

    fn mul(mut self, rhs: Self) -> Self::Output {
        self.pairs.extend(rhs.pairs);
        self.final_price *= rhs.final_price;
        self.trades_used.extend(rhs.trades_used);

        self
    }
}

impl Display for ExchangePrice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{:#?}", self.trades_used)?;
        writeln!(f, "{}", self.final_price.clone().to_float())
    }
}

pub type MakerTaker = (ExchangePrice, ExchangePrice);

impl<'a> SortedTrades<'a> {
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
    pub(crate) fn get_optimistic_price(
        &mut self,
        config: CexDexTradeConfig,
        _exchanges: &[CexExchange],
        block_timestamp: u64,
        pair: Pair,
        volume: &Rational,
        quality: Option<FastHashMap<CexExchange, FastHashMap<Pair, usize>>>,
        _bypass_vol: bool,
        dex_swap: &NormalizedSwap,
        tx_hash: FixedBytes<32>,
    ) -> Option<MakerTaker> {
        if pair.0 == pair.1 {
            return Some((
                ExchangePrice {
                    trades_used: vec![],
                    pairs:       vec![pair],
                    final_price: Rational::ONE,
                },
                ExchangePrice {
                    trades_used: vec![],
                    pairs:       vec![pair],
                    final_price: Rational::ONE,
                },
            ))
        }

        let res = self
            .get_optimistic_direct(
                config,
                block_timestamp,
                pair,
                volume,
                quality.as_ref(),
                dex_swap,
                tx_hash,
            )
            .or_else(|| {
                self.get_optimistic_via_intermediary(
                    config,
                    block_timestamp,
                    pair,
                    volume,
                    quality.as_ref(),
                    dex_swap,
                    tx_hash,
                )
            });

        if res.is_none() {
            tracing::debug!(target: "brontes_types::db::cex::optimistic", ?pair, "No price VMAP found for {}-{} in optimistic time window. \n Tx: {}", dex_swap.token_in.symbol, dex_swap.token_out.symbol, format_etherscan_url(&tx_hash));
        }

        res
    }

    fn get_optimistic_via_intermediary(
        &self,
        config: CexDexTradeConfig,
        block_timestamp: u64,
        pair: Pair,
        volume: &Rational,
        quality: Option<&FastHashMap<CexExchange, FastHashMap<Pair, usize>>>,
        dex_swap: &NormalizedSwap,
        tx_hash: FixedBytes<32>,
    ) -> Option<MakerTaker> {
        let intermediary_pairs = self.calculate_intermediary_addresses(&pair);

        println!(
            "Intermediary pairs for {}-{} are {:#?}",
            &dex_swap.token_in_symbol(),
            &dex_swap.token_out_symbol(),
            intermediary_pairs
        );

        self.calculate_intermediary_addresses(&pair)
            .into_iter()
            .filter_map(|intermediary| {
                let pair0 = Pair(pair.0, intermediary);
                let pair1 = Pair(intermediary, pair.1);

                // check if we have a path
                let mut has_pair0 = false;
                let mut has_pair1 = false;

                for trades in self.0.keys() {
                    has_pair0 |= **trades == pair0 || **trades == pair0.flip();
                    has_pair1 |= **trades == pair1 || **trades == pair1.flip();


                    if has_pair1 && has_pair0 {
                        break
                    }
                }

                if !(has_pair0 && has_pair1) {
                    return None
                }
                tracing::debug!(target: "brontes_types::db::cex::trades::optimistic", ?pair, ?intermediary, "trying via intermediary");

                let first_leg = self.get_optimistic_direct(
                    config,
                    block_timestamp,
                    pair0,
                    volume,
                    quality,
                    dex_swap,
                    tx_hash,
                )?;

                println!(
                    "First leg price is {} for pair {}-{}",
                    first_leg.0.final_price.clone().to_float(),
                    dex_swap.token_out_symbol(),
                    dex_swap.token_in_symbol()
                );

                let new_vol = volume * &first_leg.0.final_price;
                let second_leg = match self.get_optimistic_direct(
                    config,
                    block_timestamp,
                    pair1,
                    &new_vol,
                    quality,
                    dex_swap,
                    tx_hash,
                ) {
                    Some(leg) => leg,
                    None => {
                        println!("No second leg price for this intermediary: {:?}-{}", intermediary, swap.token_out_symbol());
                        return None;
                    }
                };

                println!(
                    "Second price is {} for pair {}-{}",
                    second_leg.0.final_price.clone().to_float(),
                    dex_swap.token_out_symbol(),
                    dex_swap.token_in_symbol()
                );


                let maker = first_leg.0  * second_leg.0;
                let taker = first_leg.1 * second_leg.1;

                println!(
                    "Price is {} for pair {}-{}",
                    maker.final_price.clone().to_float(),
                    dex_swap.token_out_symbol(),
                    dex_swap.token_in_symbol()
                );

                Some((maker, taker))
            })
            .max_by_key(|a| a.0.final_price.clone())
    }

    fn get_optimistic_direct(
        &self,
        config: CexDexTradeConfig,
        block_timestamp: u64,
        pair: Pair,
        volume: &Rational,
        quality: Option<&FastHashMap<CexExchange, FastHashMap<Pair, usize>>>,
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
                .map(|(k, v)| (*k, v.get(&pair).copied().unwrap_or(BASE_EXECUTION_QUALITY)))
                .collect::<FastHashMap<_, _>>()
        });

        let trade_data = self.get_trades(pair, dex_swap, tx_hash)?;

        let mut baskets_queue = TimeBasketQueue::new(trade_data, block_timestamp, quality_pct);

        baskets_queue.construct_time_baskets();

        while baskets_queue.volume.lt(volume) {
            if baskets_queue.get_min_time_delta(block_timestamp) >= config.optimistic_before_us
                || baskets_queue.get_max_time_delta(block_timestamp) >= config.optimistic_after_us
            {
                break
            }

            let min_expand = (baskets_queue.get_max_time_delta(block_timestamp)
                >= PRE_SCALING_DIFF)
                .then_some(TIME_STEP)
                .unwrap_or_default();

            baskets_queue.expand_time_bounds(min_expand, TIME_STEP);
        }

        let mut trades_used: Vec<CexTrades> = Vec::new();
        let mut unfilled = Rational::ZERO;

        // This pushed the unfilled to the next basket, given how we create the baskets
        // this means we will start from the baskets closest to the block time
        for basket in baskets_queue.baskets {
            let to_fill: Rational = ((&basket.volume / &baskets_queue.volume) * volume) + &unfilled;

            let (basket_trades, basket_unfilled) = basket.get_trades_used(&to_fill);

            unfilled = basket_unfilled;
            trades_used.extend(basket_trades);
        }

        let mut vxp_maker = Rational::ZERO;
        let mut vxp_taker = Rational::ZERO;
        let mut trade_volume = Rational::ZERO;

        let mut optimistic_trades = Vec::with_capacity(trades_used.len());

        for trade in trades_used {
            let (m_fee, t_fee) = trade.exchange.fees();

            vxp_maker += (&trade.price * (Rational::ONE - m_fee)) * &trade.amount;
            vxp_taker += (&trade.price * (Rational::ONE - t_fee)) * &trade.amount;
            trade_volume += &trade.amount;

            optimistic_trades.push(OptimisticTrade {
                volume: trade.amount.clone(),
                pair,
                price: trade.price.clone(),
                exchange: trade.exchange,
                timestamp: trade.timestamp,
            });
        }

        if trade_volume == Rational::ZERO {
            return None
        }

        let maker = ExchangePrice {
            trades_used: optimistic_trades.clone(),
            pairs:       vec![pair],
            final_price: vxp_maker / &trade_volume,
        };

        let taker = ExchangePrice {
            trades_used: optimistic_trades,
            pairs:       vec![pair],
            final_price: vxp_taker / &trade_volume,
        };

        Some((maker, taker))
    }

    pub fn get_trades(
        &'a self,
        pair: Pair,
        dex_swap: &NormalizedSwap,
        tx_hash: FixedBytes<32>,
    ) -> Option<OptimisticTradeData> {
        if let Some((indices, trades)) = self.0.get(&pair) {
            let adjusted_trades = trades
                .iter()
                .map(|trade| {
                    let adjusted_trade = trade.adjust_for_direction(Direction::Sell);
                    adjusted_trade
                })
                .collect_vec();

            Some(OptimisticTradeData {
                indices:   indices.clone(),
                trades:    adjusted_trades,
                direction: Direction::Sell,
            })
        } else {
            let flipped_pair = pair.flip();

            if let Some((indices, trades)) = self.0.get(&flipped_pair) {
                let adjusted_trades = trades
                    .iter()
                    .map(|trade| {
                        let adjusted_trade = trade.adjust_for_direction(Direction::Buy);
                        adjusted_trade
                    })
                    .collect_vec();

                Some(OptimisticTradeData {
                    indices:   indices.clone(),
                    trades:    adjusted_trades,
                    direction: Direction::Buy,
                })
            } else {
                log_missing_trade_data(dex_swap, &tx_hash);
                None
            }
        }
    }
}

pub struct Trades<'a> {
    pub trades:    Vec<(CexExchange, Vec<&'a CexTrades>)>,
    pub direction: Direction,
}

pub struct OptimisticTradeData {
    pub indices:   (usize, usize),
    pub trades:    Vec<CexTrades>,
    pub direction: Direction,
}
