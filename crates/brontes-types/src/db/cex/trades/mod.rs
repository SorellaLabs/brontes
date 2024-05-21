pub mod cex_trades;
pub mod config;
pub mod raw_cex_trades;
pub mod time_window_vwam;
pub mod utils;
pub mod vwam;

use alloy_primitives::FixedBytes;
pub use cex_trades::*;
use malachite::Rational;
pub use raw_cex_trades::*;
use time_window_vwam::TimeWindowTrades;

use self::{config::CexDexTradeConfig, time_window_vwam::MakerTakerWindowVWAP};
use super::{vwam::MakerTaker, CexExchange};
use crate::{normalized_actions::NormalizedSwap, pair::Pair, FastHashMap};

impl CexTradeMap {
    /// Calculate the price of a pair with a given volume using both the dynamic
    /// time window VMAP method & the optimistic VMAP that selects the best
    /// trades for a given time interval & volume.
    //TODO: Allow custom max pre & post TW via cli or config file
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
        let vwam = self.get_optimistic_vmap(
            config,
            exchanges,
            &pair,
            volume,
            block_timestamp,
            quality,
            bypass_vol,
            dex_swap,
            tx_hash,
        );
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
