use std::collections::HashMap;

use alloy_primitives::Address;
use brontes_types::extra_processing::Pair;
use brontes_types::normalized_actions::Actions;

#[derive(Debug, Clone)]
pub struct PoolKey {
    pub pool:   Address,
    pub run:    u128,
    pub batch:  u128,
    pub tx_idx: usize,
}

#[derive(Debug, Clone)]
pub struct DexQuotes(Vec<Option<HashMap<Pair, Vec<Vec<PoolKey>>>>>);


pub enum PoolState {
    UniswapV2(),
    UniswapV3(),
}

impl PoolState {
    pub fn update_event(&mut self, action: Actions) {}
}
