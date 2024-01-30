use std::fmt::Debug;

use alloy_primitives::{Address, Log};
use brontes_types::{normalized_actions::Actions, pair::Pair};
use malachite::Rational;

use crate::{
    errors::ArithmeticError, uniswap_v2::UniswapV2Pool, uniswap_v3::UniswapV3Pool,
    AutomatedMarketMaker, Protocol,
};

pub trait ProtocolState: Debug {
    fn price(&self, base: Address) -> Result<Rational, ArithmeticError>;
    fn tvl(&self, base: Address) -> (Rational, Rational);
}

impl ProtocolState for PoolState {
    fn tvl(&self, base: Address) -> (Rational, Rational) {
        self.get_tvl(base)
    }

    fn price(&self, base: Address) -> Result<Rational, ArithmeticError> {
        self.get_price(base)
    }
}

#[derive(Clone)]
pub struct PoolState {
    variant:         PoolVariants,
    pub last_update: u64,
}
impl Debug for PoolState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pool State")
            .field("addr", &self.address())
            .field("pair", &self.pair())
            .field("tvl 0", &self.get_tvl(self.pair().0).0)
            .field("tvl 1", &self.get_tvl(self.pair().0).1)
            .field("block", &self.last_update)
            .finish()
    }
}

impl PoolState {
    pub fn new(variant: PoolVariants, last_update: u64) -> Self {
        Self { variant, last_update }
    }

    pub fn pair(&self) -> Pair {
        match &self.variant {
            PoolVariants::UniswapV2(v) => Pair(v.token_a, v.token_b),
            PoolVariants::UniswapV3(v) => Pair(v.token_a, v.token_b),
        }
    }

    pub fn dex(&self) -> Protocol {
        match &self.variant {
            PoolVariants::UniswapV2(_) => Protocol::UniswapV2,
            PoolVariants::UniswapV3(_) => Protocol::UniswapV3,
        }
    }

    pub fn increment_state(&mut self, state: PoolUpdate) {
        self.last_update = state.block;
        self.variant.increment_state(state.logs);
    }

    pub fn address(&self) -> Address {
        match &self.variant {
            PoolVariants::UniswapV2(v) => v.address(),
            PoolVariants::UniswapV3(v) => v.address(),
        }
    }

    pub fn get_tvl(&self, base: Address) -> (Rational, Rational) {
        match &self.variant {
            PoolVariants::UniswapV2(v) => v.get_tvl(base),
            PoolVariants::UniswapV3(v) => v.get_tvl(base),
        }
    }

    pub fn get_price(&self, base: Address) -> Result<Rational, ArithmeticError> {
        match &self.variant {
            PoolVariants::UniswapV2(v) => v.calculate_price(base),
            PoolVariants::UniswapV3(v) => v.calculate_price(base),
        }
    }
}

#[derive(Debug, Clone)]
pub enum PoolVariants {
    UniswapV2(UniswapV2Pool), // 104
    UniswapV3(UniswapV3Pool), // 192,
}

impl PoolVariants {
    fn increment_state(&mut self, logs: Vec<Log>) {
        for log in logs {
            let _ = match self {
                PoolVariants::UniswapV3(a) => a.sync_from_log(log),
                PoolVariants::UniswapV2(a) => a.sync_from_log(log),
            };
        }
    }
}

#[derive(Debug, Clone)]
pub enum DexPriceMsg {
    Update(PoolUpdate),
    DiscoveredPool(DiscoveredPool, u64),
    Closed,
}

#[derive(Debug, Clone)]
pub struct DiscoveredPool {
    pub protocol:     Protocol,
    pub pool_address: Address,
    pub tokens:       Vec<Address>,
}

impl DiscoveredPool {
    pub fn new(tokens: Vec<Address>, pool_address: Address, protocol: Protocol) -> Self {
        Self { protocol, pool_address, tokens }
    }
}

#[derive(Debug, Clone)]
pub struct PoolUpdate {
    pub block:  u64,
    pub tx_idx: u64,
    pub logs:   Vec<Log>,
    pub action: Actions,
}

impl PoolUpdate {
    pub fn get_pool_address(&self) -> Address {
        self.action.get_to_address()
    }

    // we currently only use this in order to fetch the pair for when its new or to
    // fetch all pairs of it. this
    pub fn get_pair(&self, quote: Address) -> Option<Pair> {
        match &self.action {
            Actions::Swap(s) => Some(Pair(s.token_in, s.token_out)),
            Actions::Mint(m) => Some(Pair(m.token[0], m.token[1])),
            Actions::Burn(b) => Some(Pair(b.token[0], b.token[1])),
            Actions::Collect(b) => Some(Pair(b.token[0], b.token[1])),
            Actions::Transfer(t) => Some(Pair(t.token, quote)),
            Actions::Liquidation(l) => Some(Pair(l.collateral_asset, l.debt_asset)),
            Actions::SwapWithFee(s) => Some(Pair(s.token_in, s.token_out)),
            _ => None,
        }
    }
}
