use brontes_types::db::cex::CexExchange;

#[derive(Debug, Clone)]
pub struct CexDownloadConfig {
    pub block_window: (u64, u64),

    pub exchanges_to_use: Vec<CexExchange>,
}

impl CexDownloadConfig {
    pub fn new(block_window: (u64, u64), exchanges_to_use: Vec<CexExchange>) -> Self {
        Self { block_window, exchanges_to_use }
    }
}

#[cfg(not(feature = "cex-dex-quotes"))]
impl Default for CexDownloadConfig {
    fn default() -> Self {
        Self {
            block_window:     (3, 3),
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

#[cfg(feature = "cex-dex-quotes")]
impl Default for CexDownloadConfig {
    fn default() -> Self {
        Self {
            block_window:     (3, 3),
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
