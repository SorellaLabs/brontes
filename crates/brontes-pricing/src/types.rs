use std::{collections::HashMap, str::FromStr, sync::Arc};

use alloy_primitives::{Address, Log, U256};
use alloy_rlp::{Decodable, Encodable, RlpDecodable, RlpEncodable};
use brontes_types::{
    exchanges::StaticBindingsDb,
    extra_processing::Pair,
    impl_compress_decompress_for_encoded_decoded,
    libmdbx::{dex_price_mapping::DexQuoteLibmdbx, serde::address_string},
    normalized_actions::Actions,
};
use bytes::BufMut;
use malachite::{num::basic::traits::Zero, Rational};
use reth_codecs::derive_arbitrary;
use serde::{Deserialize, Serialize};
use serde_with::DisplayFromStr;
use tracing::warn;

use crate::{
    errors::ArithmeticError, graphs::PoolPairInfoDirection, uniswap_v2::UniswapV2Pool,
    uniswap_v3::UniswapV3Pool, AutomatedMarketMaker,
};

impl DexQuotes {
    pub fn price_after(&self, pair: Pair, tx: usize) -> Option<Rational> {
        if pair.0 == pair.1 {
            return Some(Rational::from(1))
        }
        self.get_price(pair, tx).cloned()
    }
}


#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DexQuotes(pub Vec<Option<HashMap<Pair, Rational>>>);

impl DexQuotes {
    pub fn get_price(&self, pair: Pair, tx: usize) -> Option<&Rational> {
        self.0.get(tx)?.as_ref()?.get(&pair)
    }
}

pub(crate) trait ProtocolState {
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

#[derive(Debug, Clone)]
pub struct PoolState {
    variant: PoolVariants,
}

impl PoolState {
    pub fn new(variant: PoolVariants) -> Self {
        Self { variant }
    }

    pub fn pair(&self) -> Pair {
        match &self.variant {
            PoolVariants::UniswapV2(v) => Pair(v.token_a, v.token_b),
            PoolVariants::UniswapV3(v) => Pair(v.token_a, v.token_b),
        }
    }

    pub fn dex(&self) -> StaticBindingsDb {
        match &self.variant {
            PoolVariants::UniswapV2(_) => StaticBindingsDb::UniswapV2,
            PoolVariants::UniswapV3(_) => StaticBindingsDb::UniswapV3,
        }
    }

    pub fn increment_state(&mut self, state: PoolUpdate) {
        self.variant.increment_state(state.action, state.logs);
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
    UniswapV2(UniswapV2Pool),
    UniswapV3(UniswapV3Pool),
}

impl PoolVariants {
    fn increment_state(&mut self, _action: Actions, logs: Vec<Log>) {
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
    Closed,
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
            _ => None,
        }
    }
}

impl From<Vec<DexQuoteLibmdbx>> for DexQuotes {
    fn from(value: Vec<DexQuoteLibmdbx>) -> Self {
        todo!()
    }
}
