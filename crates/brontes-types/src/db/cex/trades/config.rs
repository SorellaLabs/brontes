#[derive(Debug, Clone, Copy)]
pub struct CexDexTradeConfig {
    pub initial_vwap_pre_block_us:         u64,
    pub initial_vwap_post_block_us:        u64,
    pub max_vwap_pre_block_us:             u64,
    pub max_vwap_post_block_us:            u64,
    pub vwap_scaling_diff_us:              u64,
    pub vwap_time_step_us:                 u64,
    pub use_block_time_weights_vwap:       bool,
    pub pre_decay_weight_vwap:             f64,
    pub post_decay_weight_vwap:            f64,
    pub initial_optimistic_pre_block_us:   u64,
    pub initial_optimistic_post_block_us:  u64,
    pub max_optimistic_pre_block_us:       u64,
    pub max_optimistic_post_block_us:      u64,
    pub optimistic_scaling_diff_us:        u64,
    pub optimistic_time_step_us:           u64,
    pub use_block_time_weights_optimistic: bool,
    pub pre_decay_weight_op:               f64,
    pub post_decay_weight_op:              f64,
    pub quote_offset_from_block_us:        u64,
}

impl Default for CexDexTradeConfig {
    fn default() -> Self {
        Self {
            initial_vwap_pre_block_us:         50_000,
            initial_vwap_post_block_us:        50_000,
            max_vwap_pre_block_us:             10_000_000,
            max_vwap_post_block_us:            20_000_000,
            vwap_scaling_diff_us:              300_000,
            vwap_time_step_us:                 10_000,
            use_block_time_weights_vwap:       false,
            pre_decay_weight_vwap:             -0.0000005,
            post_decay_weight_vwap:            -0.0000002,
            initial_optimistic_pre_block_us:   300_000,
            initial_optimistic_post_block_us:  50_000,
            max_optimistic_pre_block_us:       500_000,
            max_optimistic_post_block_us:      4_000_000,
            optimistic_scaling_diff_us:        0,
            optimistic_time_step_us:           100_000,
            use_block_time_weights_optimistic: false,
            pre_decay_weight_op:               -0.0000003,
            post_decay_weight_op:              -0.00000012,
            quote_offset_from_block_us:        0,
        }
    }
}

impl CexDexTradeConfig {
    pub fn with_block_time_weights(&mut self) {
        self.use_block_time_weights_optimistic = true;
        self.use_block_time_weights_vwap = true;
    }
}
