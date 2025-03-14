use std::fmt::{Debug, Display};

use alloy_primitives::{wrap_fixed_bytes, Address, FixedBytes, Log};
use brontes_types::{
    constants::WETH_ADDRESS,
    normalized_actions::{pool::NormalizedPoolConfigUpdate, Action},
    pair::Pair,
};
use malachite::Rational;

use crate::{
    errors::ArithmeticError, uniswap_v2::UniswapV2Pool, uniswap_v3::UniswapV3Pool, LoadState,
    Protocol, UpdatableProtocol,
};

wrap_fixed_bytes!(extra_derives:[],
                  pub struct PairWithFirstPoolHop<80>;);

impl Display for PairWithFirstPoolHop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (pair, gt) = self.pair_gt();
        write!(f, "pair={:?}, goes_through={:?}", pair, gt)
    }
}

impl PairWithFirstPoolHop {
    pub fn from_pair_gt(pair: Pair, goes_through: Pair) -> Self {
        let mut buf = [0u8; 80];
        buf[0..20].copy_from_slice(&**pair.0);
        buf[20..40].copy_from_slice(&**pair.1);
        buf[40..60].copy_from_slice(&**goes_through.0);
        buf[60..80].copy_from_slice(&**goes_through.1);

        Self(FixedBytes::new(buf))
    }

    pub fn get_pair(&self) -> Pair {
        let addr0 = Address::from_slice(&self.0[0..20]);
        let addr1 = Address::from_slice(&self.0[20..40]);
        Pair(addr0, addr1)
    }

    pub fn get_goes_through(&self) -> Pair {
        let addr0 = Address::from_slice(&self.0[40..60]);
        let addr1 = Address::from_slice(&self.0[60..80]);
        Pair(addr0, addr1)
    }

    pub fn pair_gt(&self) -> (Pair, Pair) {
        (self.get_pair(), self.get_goes_through())
    }
}

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
        if !state.is_supported_protocol() {
            tracing::error!(state_transition=?state, "tried to apply a invalid state transition");
            return;
        }
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
    UniswapV2(Box<UniswapV2Pool>),
    UniswapV3(Box<UniswapV3Pool>),
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
    /// marker for only updating loaded state and not generating prices
    DisablePricingFor(u64),
    Update(PoolUpdate),
    /// we only send pool config update if the pool is valid and has tokens
    DiscoveredPool(NormalizedPoolConfigUpdate),
    Closed,
}

impl DexPriceMsg {
    pub fn get_action(&self) -> Action {
        match self {
            Self::Update(u) => u.action.clone(),
            Self::DiscoveredPool(p) => Action::PoolConfigUpdate(p.clone()),
            _ => unreachable!("called get action on closed msg"),
        }
    }
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
    pub action: Action,
}

impl PoolUpdate {
    pub fn get_pool_address(&self) -> Address {
        self.action.get_to_address()
    }

    pub fn get_pool_address_for_pricing(&self) -> Option<Address> {
        // these don't have a pool address
        if self.action.is_transfer()
            || self.action.is_batch()
            || self.action.is_aggregator()
            || self.action.is_eth_transfer()
        {
            return None;
        }
        Some(self.get_pool_address())
    }

    pub fn is_transfer(&self) -> bool {
        self.action.is_transfer()
    }

    pub fn is_supported_protocol(&self) -> bool {
        if let Action::Swap(s) = &self.action {
            return s.protocol.has_state_updater();
        } else if let Action::SwapWithFee(s) = &self.action {
            return s.protocol.has_state_updater();
        }

        true
    }

    // we currently only use this in order to fetch the pair for when its new or to
    // fetch all pairs of it. this
    pub fn get_pair(&self, quote: Address) -> Option<Pair> {
        match &self.action {
            Action::Swap(s) => Some(Pair(s.token_in.address, s.token_out.address)),
            Action::Mint(m) => Some(Pair(
                m.token.first()?.address,
                m.token.get(1).map(|t| t.address).unwrap_or(quote),
            )),
            Action::Burn(b) => Some(Pair(
                b.token.first()?.address,
                b.token.get(1).map(|t| t.address).unwrap_or(quote),
            )),
            Action::Collect(b) => Some(Pair(
                b.token.first()?.address,
                b.token.get(1).map(|t| t.address).unwrap_or(quote),
            )),
            Action::Transfer(t) => Some(Pair(t.token.address, quote)),
            Action::EthTransfer(_) => Some(Pair(WETH_ADDRESS, quote)),
            Action::Liquidation(l) => Some(Pair(l.collateral_asset.address, l.debt_asset.address)),
            Action::SwapWithFee(s) => Some(Pair(s.token_in.address, s.token_out.address)),
            rest => {
                tracing::debug!(?rest, "tried to get pair for action with no def");
                None
            }
        }
    }
}
