use std::{collections::HashMap, sync::Arc};

use alloy_primitives::Address;
use brontes_types::{extra_processing::Pair, normalized_actions::Actions};
// use crate::exchanges::{uniswap_v2::UniswapV2Pool, uniswap_v3::UniswapV3Pool};
use malachite::Rational;

#[derive(Debug, Clone)]
pub struct PoolKey {
    pub pool:   Address,
    pub run:    u64,
    pub batch:  u64,
    pub tx_idx: usize,
}

pub struct DexPrices {
    quotes: DexQuotes,
    state:  Arc<HashMap<PoolKey, PoolState>>,
}

impl DexPrices {
    pub fn price_after(&self, pair: Pair, tx: usize) -> Rational {
        // self.quotes
        todo!()
    }
}

#[derive(Debug, Clone)]
pub struct DexQuotes(Vec<Option<HashMap<Pair, Vec<Vec<PoolKey>>>>>);

impl DexQuotes {
    pub fn get_pair_keys(&self, pair: Pair, tx: usize) -> &Vec<Vec<PoolKey>> {
        self.0
            .get(tx)
            .expect("this should never be reached")
            .as_ref()
            .expect("unreachable")
            .get(&pair)
            .unwrap()
    }
}

pub enum PoolStateSnapShot {
    UniswapV2(()),
    UniswapV3(()),
}

pub enum PoolState {
    UniswapV2(()),
    UniswapV3(()),
}

impl PoolState {}

pub enum PairPriceMessage {
    Update(PoolUpdate),
    Finished(u64),
}

pub struct PoolUpdate {
    pub block:  u64,
    pub tx_idx: usize,
    pub action: Actions,
}

impl PoolState {
    pub fn update_event(&mut self, action: Actions) {}
}
