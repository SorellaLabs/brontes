use brontes_types::db::cex::CexExchange;

#[derive(Debug, Clone)]
pub struct CexDownloadConfig {
    pub time_window:      (f64, f64),
    pub exchanges_to_use: Vec<CexExchange>,
}

impl CexDownloadConfig {
    pub fn new(time_window: (f64, f64), exchanges_to_use: Vec<CexExchange>) -> Self {
        Self { time_window, exchanges_to_use }
    }
}

#[cfg(feature = "cex-dex-markout")]
impl Default for CexDownloadConfig {
    fn default() -> Self {
        Self {
            time_window:      (0.5, 2.0),
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
            time_window:      (0.5, 1.0),
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
