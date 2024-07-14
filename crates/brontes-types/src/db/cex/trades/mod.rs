pub mod cex_trades;
pub mod config;
pub mod optimistic;
pub mod raw_cex_trades;
pub mod time_window_vwam;
pub mod utils;

use alloy_primitives::FixedBytes;
pub use cex_trades::*;
use malachite::Rational;
pub use raw_cex_trades::*;
use time_window_vwam::TimeWindowTrades;

use self::{
    config::CexDexTradeConfig, time_window_vwam::MakerTakerWindowVWAP, utils::SortedTrades,
};
use super::{optimistic::MakerTaker, CexExchange};
use crate::{normalized_actions::NormalizedSwap, pair::Pair, FastHashMap};

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
    ) -> (Option<MakerTakerWindowVWAP>, Option<MakerTaker>) {
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
        &mut self,
        config: CexDexTradeConfig,
        exchanges: &[CexExchange],
        pair: Pair,
        volume: &Rational,
        block_timestamp: u64,
        bypass_vol: bool,
        dex_swap: &NormalizedSwap,
        tx_hash: FixedBytes<32>,
    ) -> Option<MakerTakerWindowVWAP> {
        TimeWindowTrades::new_from_cex_trade_map(&mut self.0, block_timestamp, exchanges, pair)
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

    pub fn get_optimistic_vmap(
        &mut self,
        config: CexDexTradeConfig,
        exchanges: &[CexExchange],
        pair: Pair,
        volume: &Rational,
        block_timestamp: u64,
        quality: Option<FastHashMap<CexExchange, FastHashMap<Pair, usize>>>,
        bypass_vol: bool,
        dex_swap: &NormalizedSwap,
        tx_hash: FixedBytes<32>,
    ) -> Option<MakerTaker> {
        SortedTrades::new_from_cex_trade_map(&mut self.0, exchanges, pair, block_timestamp)
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
