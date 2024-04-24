pub mod cex_trades;
pub mod raw_cex_trades;
pub mod time_window_vwam;
pub mod utils;
pub mod vwam;

pub use cex_trades::*;
use malachite::Rational;
pub use raw_cex_trades::*;
use time_window_vwam::TimeWindowTrades;

use self::time_window_vwam::MakerTakerWindowVWAP;
use super::{vwam::MakerTaker, CexExchange};
use crate::{pair::Pair, FastHashMap};

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
    ) -> (Option<MakerTakerWindowVWAP>, Option<MakerTaker>) {
        let vwam = self.get_optimistic_vmap(exchanges, &pair, volume, quality);
        let window = self.calculate_time_window_vwam(exchanges, pair, volume, block_timestamp);

        (window, vwam)
    }

    pub fn calculate_time_window_vwam(
        &mut self,
        exchanges: &[CexExchange],
        pair: Pair,
        volume: &Rational,
        block_timestamp: u64,
    ) -> Option<MakerTakerWindowVWAP> {
        TimeWindowTrades::new_from_cex_trade_map(&mut self.0, block_timestamp, exchanges, pair)
            .get_price(exchanges, pair, volume, block_timestamp)
    }

    pub fn get_optimistic_vmap(
        &mut self,
        exchanges: &[CexExchange],
        pair: &Pair,
        volume: &Rational,
        quality: Option<FastHashMap<CexExchange, FastHashMap<Pair, usize>>>,
    ) -> Option<MakerTaker> {
        self.get_price(exchanges, pair, volume, quality)
    }
}
