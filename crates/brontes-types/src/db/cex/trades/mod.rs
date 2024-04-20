pub mod cex_trades;
pub mod raw_cex_trades;
pub mod time_window_vwam;
pub mod utils;
pub mod vwam;

pub use cex_trades::*;
use malachite::Rational;
pub use raw_cex_trades::*;
use time_window_vwam::TimeWindowTrades;

use self::time_window_vwam::MakerTakerWindowVwam;
use super::{vwam::MakerTaker, CexExchange};
use crate::{pair::Pair, FastHashMap};

/// impl for generating both prices
impl CexTradeMap {
    pub fn calculate_all_methods(
        &mut self,
        exchanges: &[CexExchange],
        pair: Pair,
        volume: &Rational,
        timestamp: u64,
        quality: Option<FastHashMap<CexExchange, FastHashMap<Pair, usize>>>,
    ) -> (Option<MakerTakerWindowVwam>, Option<MakerTaker>) {
        let vwam = self.get_price_vwam(exchanges, &pair, volume, quality);
        let window = self.calculate_time_window_vwam(exchanges, pair, volume, timestamp);

        (window, vwam)
    }

    pub fn calculate_time_window_vwam(
        &mut self,
        exchanges: &[CexExchange],
        pair: Pair,
        volume: &Rational,
        timestamp: u64,
    ) -> Option<MakerTakerWindowVwam> {
        TimeWindowTrades::new_from_cex_trade_map(&mut self.0, timestamp, exchanges, pair)
            .get_price(exchanges, pair, volume, timestamp)
    }

    pub fn get_price_vwam(
        &mut self,
        exchanges: &[CexExchange],
        pair: &Pair,
        volume: &Rational,
        quality: Option<FastHashMap<CexExchange, FastHashMap<Pair, usize>>>,
    ) -> Option<MakerTaker> {
        self.get_price(exchanges, pair, volume, quality)
    }
}
