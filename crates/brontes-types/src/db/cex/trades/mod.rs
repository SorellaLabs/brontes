pub mod cex_trades;
pub mod config;
pub mod optimistic;
pub mod raw_cex_trades;
pub mod time_window_vwam;
pub mod utils;

use alloy_primitives::{Address, FixedBytes};
pub use cex_trades::*;
use malachite::Rational;
pub use raw_cex_trades::*;
use time_window_vwam::TimeWindowTrades;

use self::{config::CexDexTradeConfig, time_window_vwam::WindowExchangePrice, utils::SortedTrades};
use super::{optimistic::OptimisticPrice, CexExchange};
use crate::{constants::WETH_ADDRESS, normalized_actions::NormalizedSwap, pair::Pair, FastHashMap};

impl CexTradeMap {
    /// Calculate the price of a pair with a given volume using both the dynamic
    /// time window VWAP method & the optimistic VWAP that selects the best
    /// trades for a given time interval & volume.
    pub fn calculate_all_methods(
        &mut self,
        exchanges: &[CexExchange],
        pair: Pair,
        volume: &Rational,
        block_timestamp: u64,
        quality: Option<FastHashMap<CexExchange, FastHashMap<Pair, usize>>>,
        bypass_vol: bool,
        dex_swap: &NormalizedSwap,
        tx_hash: FixedBytes<32>,
        config: CexDexTradeConfig,
    ) -> (Option<WindowExchangePriceP>, Option<OptimisticPrice>) {
        let window = self.calculate_time_window_vwam(
            config,
            exchanges,
            pair,
            volume,
            block_timestamp,
            bypass_vol,
            dex_swap,
            tx_hash,
        );

        let vwam = self.get_optimistic_vmap(
            config,
            exchanges,
            pair,
            volume,
            block_timestamp,
            quality,
            bypass_vol,
            dex_swap,
            tx_hash,
        );

        (window, vwam)
    }

    pub fn calculate_time_window_vwam(
        &self,
        config: CexDexTradeConfig,
        exchanges: &[CexExchange],
        pair: Pair,
        volume: &Rational,
        block_timestamp: u64,
        bypass_vol: bool,
        dex_swap: &NormalizedSwap,
        tx_hash: FixedBytes<32>,
    ) -> Option<WindowExchangePrice> {
        TimeWindowTrades::new_from_cex_trade_map(&self.0, block_timestamp, exchanges, pair)
            .get_price(
                config,
                exchanges,
                pair,
                volume,
                block_timestamp,
                bypass_vol,
                dex_swap,
                tx_hash,
            )
    }

    /// Gets the Binance ETH price at the block time. This is used to calculate
    /// the transaction costs.
    pub fn get_eth_price(
        &mut self,
        block_timestamp: u64,
        quote_asset: Address,
    ) -> Option<Rational> {
        let trades = self
            .0
            .get_mut(&CexExchange::Binance)?
            .get_mut(&Pair(WETH_ADDRESS, quote_asset))?;

        trades.sort_unstable_by_key(|t| t.timestamp);

        let index = trades.partition_point(|t| t.timestamp < block_timestamp);

        let relevant_trades = trades.iter().skip(index).take(5);

        let (sum, count) = relevant_trades
            .fold((Rational::from(0), 0), |(sum, count), trade| (sum + &trade.price, count + 1));

        if count == 0 {
            None
        } else {
            Some(sum / Rational::from(count))
        }
    }

    pub fn get_optimistic_vmap(
        &self,
        config: CexDexTradeConfig,
        exchanges: &[CexExchange],
        pair: Pair,
        volume: &Rational,
        block_timestamp: u64,
        quality: Option<FastHashMap<CexExchange, FastHashMap<Pair, usize>>>,
        bypass_vol: bool,
        dex_swap: &NormalizedSwap,
        tx_hash: FixedBytes<32>,
    ) -> Option<OptimisticPrice> {
        SortedTrades::new_from_cex_trade_map(&self.0, exchanges, pair, block_timestamp)
            .get_optimistic_price(
                config,
                exchanges,
                block_timestamp,
                pair,
                volume,
                quality,
                bypass_vol,
                dex_swap,
                tx_hash,
            )
    }
}
