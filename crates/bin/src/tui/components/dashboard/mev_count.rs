#[derive(Default, Debug)]
pub struct MevCount {
    pub sandwich_count:       u64,
    pub cex_dex_count:        u64,
    pub jit_count:            u64,
    pub jit_sandwich_count:   u64,
    pub atomic_backrun_count: u64,
    pub liquidation_count:    u64,
}
