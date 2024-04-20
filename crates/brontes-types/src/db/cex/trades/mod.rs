pub mod cex_trades;
pub mod raw_cex_trades;
pub mod time_window_vwam;
pub mod utils;
pub mod vwam;

use cex_trades::CexTradeMap;
pub use cex_trades::*;
use malachite::Rational;
pub use raw_cex_trades::*;

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
    ) {
    }

    pub fn calculate_time_window_vwam(
        &mut self,
        exchanges: &[CexExchange],
        pair: Pair,
        volume: &Rational,
        timestamp: u64,
    ) {
    }

    pub fn get_price_vwam(
        &self,
        exchanges: &[CexExchange],
        pair: &Pair,
        volume: &Rational,
        quality: Option<FastHashMap<CexExchange, FastHashMap<Pair, usize>>>,
    ) -> Option<MakerTaker> {
        None
    }
}
