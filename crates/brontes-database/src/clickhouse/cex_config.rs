use brontes_types::db::cex::CexExchange;

#[derive(Debug, Clone)]
pub struct CexDownloadConfig {
    pub time_window:      (u64, u64),
    pub exchanges_to_use: Vec<CexExchange>,
}

impl CexDownloadConfig {
    pub fn new(time_window: (u64, u64), exchanges_to_use: Vec<CexExchange>) -> Self {
        Self { time_window, exchanges_to_use }
    }
}

#[cfg(feature = "cex-dex-markout")]
impl Default for CexDownloadConfig {
    fn default() -> Self {
        Self {
            time_window:      (6, 6),
            exchanges_to_use: vec![
                CexExchange::Binance,
                CexExchange::Coinbase,
                CexExchange::Okex,
                CexExchange::BybitSpot,
                CexExchange::Kucoin,
            ],
        }
    }
}

#[cfg(not(feature = "cex-dex-markout"))]
impl Default for CexDownloadConfig {
    fn default() -> Self {
        Self {
            time_window:      (12, 0),
            exchanges_to_use: vec![
                CexExchange::Binance,
                CexExchange::Coinbase,
                CexExchange::Okex,
                CexExchange::BybitSpot,
                CexExchange::Kucoin,
            ],
        }
    }
}
