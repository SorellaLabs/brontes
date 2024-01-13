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

#[derive(
    Debug,
    Default,
    Clone,
    Hash,
    PartialEq,
    Eq,
    Copy,
    Serialize,
    Deserialize,
    Ord,
    PartialOrd,
    RlpEncodable,
    RlpDecodable,
)]
pub struct PoolKey {
    #[serde(with = "address_string")]
    pub pool:         Address,
    pub run:          u64,
    pub batch:        u64,
    pub update_nonce: u16,
}

impl FromStr for PoolKey {
    type Err = u8;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let split = s.split('-').collect::<Vec<_>>();
        let pool = Address::from_str(split[0]).unwrap();
        let run = u64::from_str(split[1]).unwrap();
        let batch = u64::from_str(split[2]).unwrap();
        let update_nonce = u16::from_str(split[3]).unwrap();

        Ok(Self { update_nonce, pool, run, batch })
    }
}

impl reth_db::table::Decode for PoolKey {
    fn decode<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
        let binding = value.as_ref().to_vec();
        let buf = &mut binding.as_slice();
        <PoolKey as Decodable>::decode(buf).map_err(|e| reth_db::DatabaseError::Decode)
    }
}

impl reth_db::table::Encode for PoolKey {
    type Encoded = Vec<u8>;

    fn encode(self) -> Self::Encoded {
        let mut buf = Vec::new();
        <PoolKey as Encodable>::encode(&self, &mut buf);
        buf
    }
}

impl_compress_decompress_for_encoded_decoded!(PoolKey);

impl DexQuotes {
    pub fn price_after(&self, pair: Pair, tx: usize) -> Option<Rational> {
        if pair.0 == pair.1 {
            return Some(Rational::from(1))
        }
        self.get_price(pair, tx).cloned()
    }
}

#[derive(
    Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, RlpEncodable, RlpDecodable,
)]
pub struct PoolKeyWithDirection {
    pub key:  PoolKey,
    pub base: Address,
}

impl PoolKeyWithDirection {
    pub fn new(key: PoolKey, base: Address) -> Self {
        Self { key, base }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, RlpEncodable, RlpDecodable)]
pub struct PoolKeysForPair(pub Vec<PoolKeyWithDirection>);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DexQuotes(pub Vec<Option<HashMap<Pair, Rational>>>);

impl DexQuotes {
    pub fn get_price(&self, pair: Pair, tx: usize) -> Option<&Rational> {
        self.0.get(tx)?.as_ref()?.get(&pair)
    }
}

/// a immutable version of pool state that is for a specific post-transition
/// period
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PoolStateSnapShot {
    UniswapV2(UniswapV2Pool),
    UniswapV3(UniswapV3Pool),
}

impl_compress_decompress_for_encoded_decoded!(PoolStateSnapShot);

impl PoolStateSnapShot {
    pub fn get_tvl(&self, base: Address) -> (Rational, Rational) {
        match self {
            PoolStateSnapShot::UniswapV2(v) => v.get_tvl(base),
            PoolStateSnapShot::UniswapV3(v) => v.get_tvl(base),
        }
    }

    pub fn get_price(&self, base: Address) -> Rational {
        match self {
            PoolStateSnapShot::UniswapV2(v) => {
                Rational::try_from(v.calculate_price(base).unwrap()).unwrap()
            }
            PoolStateSnapShot::UniswapV3(v) => {
                let price = v.calculate_price(base);
                if price.is_err() {
                    tracing::error!(?price, "failed to get price");
                    return Rational::ZERO
                }

                Rational::try_from(price.unwrap()).unwrap()
            }
        }
    }

    pub fn get_base_token(&self, token_0_in: bool) -> Address {
        match self {
            PoolStateSnapShot::UniswapV3(v) => {
                if token_0_in {
                    v.token_a
                } else {
                    v.token_b
                }
            }
            PoolStateSnapShot::UniswapV2(v) => {
                if token_0_in {
                    v.token_a
                } else {
                    v.token_b
                }
            }
        }
    }

    /// for encoding help
    fn variant(&self) -> u8 {
        match self {
            PoolStateSnapShot::UniswapV2(_) => 0,
            PoolStateSnapShot::UniswapV3(_) => 1,
        }
    }
}

impl Encodable for PoolStateSnapShot {
    fn encode(&self, out: &mut dyn BufMut) {
        self.variant().encode(out);
        match self {
            PoolStateSnapShot::UniswapV2(st) => st.encode(out),
            PoolStateSnapShot::UniswapV3(st) => st.encode(out),
        }
    }
}

impl Decodable for PoolStateSnapShot {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let variant = u8::decode(buf)?;

        let this = match variant {
            0 => PoolStateSnapShot::UniswapV2(UniswapV2Pool::decode(buf)?),
            1 => PoolStateSnapShot::UniswapV3(UniswapV3Pool::decode(buf)?),
            _ => unreachable!("can't decode this variant"),
        };

        Ok(this)
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
        // match self {
        //     PoolVariants::UniswapV3(a) =>
        // a.sync_from_action(action).unwrap(),
        //     PoolVariants::UniswapV2(a) =>
        // a.sync_from_action(action).unwrap(), }
    }

    fn into_snapshot(self) -> PoolStateSnapShot {
        match self {
            Self::UniswapV2(v) => PoolStateSnapShot::UniswapV2(v),
            Self::UniswapV3(v) => PoolStateSnapShot::UniswapV3(v),
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
