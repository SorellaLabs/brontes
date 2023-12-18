use std::{collections::HashMap, sync::Arc};

use alloy_primitives::Address;
use brontes_types::{extra_processing::Pair, normalized_actions::Actions};
// use crate::exchanges::{uniswap_v2::UniswapV2Pool, uniswap_v3::UniswapV3Pool};
use malachite::Rational;

#[derive(Debug, Clone, Hash, PartialEq, Eq, Copy)]
pub struct PoolKey {
    pub pool:         Address,
    pub run:          u64,
    pub batch:        u64,
    pub update_nonce: u16,
}

#[derive(Debug, Clone)]
pub struct DexPrices {
    pub(crate) quotes: DexQuotes,
    pub(crate) state:  Arc<HashMap<PoolKey, PoolStateSnapShot>>,
}

impl DexPrices {
    pub fn new() -> Self {
        todo!()
    }

    pub fn price_after(&self, pair: Pair, tx: usize) -> Rational {
        // self.quotes
        todo!()
    }
}

#[derive(Debug, Clone)]
pub struct DexQuotes(pub(crate) Vec<Option<HashMap<Pair, Vec<PoolKey>>>>);

impl DexQuotes {
    pub fn get_pair_keys(&self, pair: Pair, tx: usize) -> &Vec<PoolKey> {
        self.0
            .get(tx)
            .expect("this should never be reached")
            .as_ref()
            .expect("unreachable")
            .get(&pair)
            .unwrap()
    }
}

/// a immutable version of pool state that is for a specific post-transition
/// period
#[derive(Debug, Clone)]
pub enum PoolStateSnapShot {
    UniswapV2(()),
    UniswapV3(()),
}

pub struct PoolState {
    update_nonce: u16,
    variant:      PoolVariants,
}

impl PoolState {
    pub fn increment_state(&mut self, state: PoolUpdate) -> (u16, PoolStateSnapShot) {
        self.update_nonce += 1;
        self.variant.increment_state(state);
        (self.update_nonce, self.variant.clone().into_snapshot())
    }

    pub fn into_snapshot(&self) -> PoolStateSnapShot {
        self.variant.clone().into_snapshot()
    }

    pub fn address(&self) -> Address {
        todo!()
    }
}

#[derive(Debug, Clone)]
pub enum PoolVariants {
    UniswapV2(()),
    UniswapV3(()),
}

impl PoolVariants {
    fn increment_state(&mut self, state: PoolUpdate) {
        todo!()
    }

    fn into_snapshot(self) -> PoolStateSnapShot {
        todo!()
    }
}

#[derive(Debug, Clone)]
pub struct PoolUpdate {
    pub block:  u64,
    pub tx_idx: usize,
    pub action: Actions,
}

impl PoolUpdate {
    pub fn get_pool_address(&self) -> Address {
        self.action.get_to_address()
    }

    // we currently only use this in order to fetch the pair for when its new or to
    // fetch all pairs of it. this
    pub fn get_pair(&self) -> Option<Pair> {
        match &self.action {
            Actions::Swap(s) => Some(Pair(s.token_in, s.token_out)),
            Actions::Mint(m) => Some(Pair(m.token[0], m.token[1])),
            Actions::Burn(b) => Some(Pair(b.token[0], b.token[1])),
            _ => None,
        }
    }
}
