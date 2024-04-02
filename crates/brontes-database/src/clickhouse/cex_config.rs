use brontes_types::db::cex::CexExchange;

#[derive(Debug, Clone)]
pub struct CexDownloadConfig {
    pub prices:           bool,
    pub trades:           bool,
    pub price_window:     (u64, u64),
    pub trades_window:    (u64, u64),
    pub exchanges_to_use: Vec<CexExchange>,
}

impl CexDownloadConfig {
    pub fn new(
        price_window: (u64, u64),
        trades_window: (u64, u64),
        exchanges: Vec<CexExchange>,
    ) -> Self {
        let mut this = Self::default();
        this.price_window = price_window;
        this.trades_window = trades_window;
        this.exchanges_to_use = exchanges;

        this
    }
}

#[cfg(feature = "cex-dex-markout")]
impl Default for CexDownloadConfig {
    fn default() -> Self {
        Self {
            prices:           false,
            trades:           true,
            price_window:     (12, 0),
            trades_window:    (6, 6),
            exchanges_to_use: Vec::new(),
        }
    }
}

#[cfg(not(feature = "cex-dex-markout"))]
impl Default for CexDownloadConfig {
    fn default() -> Self {
        Self {
            prices:           true,
            trades:           false,
            price_window:     (12, 0),
            trades_window:    (6, 6),
            exchanges_to_use: Vec::new(),
        }
    }
}
