#[derive(Debug, Clone, Copy)]
pub struct CexDexTradeConfig {
    pub time_window_before_us: u64,
    pub time_window_after_us:  u64,
    pub optimistic_before_us:  u64,
    pub optimistic_after_us:   u64,
    pub quotes_fetch_time:     u64,
}

impl Default for CexDexTradeConfig {
    fn default() -> Self {
        Self {
            time_window_after_us:  8_000_000,
            time_window_before_us: 5_000_000,
            optimistic_before_us:  500_000,
            optimistic_after_us:   2_000_000,
            quotes_fetch_time:     2_000_000,
        }
    }
}
