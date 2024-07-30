use brontes_types::db::cex::CexExchange;

#[derive(Debug, Clone)]
pub struct CexDownloadConfig {
    pub run_time_window:  (u64, u64),
    pub exchanges_to_use: Vec<CexExchange>,
}

impl CexDownloadConfig {
    pub fn new(run_time_window: (u64, u64), exchanges_to_use: Vec<CexExchange>) -> Self {
        Self { run_time_window, exchanges_to_use }
    }
}

impl Default for CexDownloadConfig {
    fn default() -> Self {
        Self {
            run_time_window:  (6, 6),
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
