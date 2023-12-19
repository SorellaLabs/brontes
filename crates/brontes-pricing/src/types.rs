use std::{collections::HashMap, sync::Arc};

use alloy_primitives::Address;
use alloy_rlp::{Decodable, Encodable, RlpDecodable, RlpEncodable};
use brontes_types::{
    exchanges::StaticBindingsDb, extra_processing::Pair,
    impl_compress_decompress_for_encoded_decoded, libmdbx_utils::serde_address_string,
    normalized_actions::Actions,
};
use bytes::BufMut;
// use crate::exchanges::{uniswap_v2::UniswapV2Pool, uniswap_v3::UniswapV3Pool};
use malachite::{num::basic::traits::Zero, Rational};
use reth_rpc_types::Log;
use serde::{Deserialize, Serialize};
use serde_with::DisplayFromStr;

use crate::{
    graph::PoolPairInfoDirection, uniswap_v2::UniswapV2Pool, uniswap_v3::UniswapV3Pool,
    AutomatedMarketMaker,
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
    #[serde(with = "serde_address_string")]
    pub pool:         Address,
    pub run:          u64,
    pub batch:        u64,
    pub update_nonce: u16,
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

/// Block level pre-computed prices for all dexes
/// Generated by our price fetcher and stored in libmdbx for fast reruns
#[derive(Debug, Clone)]
pub struct DexPrices {
    pub(crate) quotes: DexQuotes,
    pub(crate) state:  Arc<HashMap<PoolKey, PoolStateSnapShot>>,
}

impl DexPrices {
    pub fn new(state: Arc<HashMap<PoolKey, PoolStateSnapShot>>, quotes: DexQuotes) -> Self {
        Self { state, quotes }
    }

    pub fn price_after(&self, pair: Pair, tx: usize) -> Rational {
        let keys = self.quotes.get_pair_keys(pair, tx);
        let mut price = Rational::ZERO;

        for hop in keys {
            let mut pxw = Rational::ZERO;
            let mut weight = Rational::ZERO;

            for hop_pool in &hop.0 {
                let pair_detail = self.state.get(&hop_pool.key).unwrap();
                let res = pair_detail.get_price(hop_pool.base);
                let tvl = pair_detail.get_tvl();

                let weight_price = res * &tvl;

                pxw += weight_price;
                weight += tvl;
            }
            if price == Rational::ZERO {
                price = pxw / weight;
            } else {
                price *= (pxw / weight);
            }
        }

        price
    }
}

#[derive(Debug, Clone)]
pub struct PoolKeyWithDirection {
    pub key:  PoolKey,
    pub base: Address,
}

impl PoolKeyWithDirection {
    pub fn new(key: PoolKey, base: Address) -> Self {
        Self { key, base }
    }
}

#[derive(Debug, Clone)]
pub struct PoolKeysForPair(pub Vec<PoolKeyWithDirection>);

#[derive(Debug, Clone)]
pub struct DexQuotes(pub Vec<Option<HashMap<Pair, Vec<PoolKeysForPair>>>>);

impl DexQuotes {
    pub fn get_pair_keys(&self, pair: Pair, tx: usize) -> &Vec<PoolKeysForPair> {
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PoolStateSnapShot {
    UniswapV2(UniswapV2Pool),
    UniswapV3(UniswapV3Pool),
}

impl_compress_decompress_for_encoded_decoded!(PoolStateSnapShot);

impl PoolStateSnapShot {
    pub fn get_tvl(&self) -> Rational {
        match self {
            PoolStateSnapShot::UniswapV2(v) => v.get_tvl(),
            PoolStateSnapShot::UniswapV3(v) => v.get_tvl(),
        }
    }

    pub fn get_price(&self, base: Address) -> Rational {
        match self {
            PoolStateSnapShot::UniswapV2(v) => {
                Rational::try_from(v.calculate_price(base).unwrap()).unwrap()
            }
            PoolStateSnapShot::UniswapV3(v) => {
                Rational::try_from(v.calculate_price(base).unwrap()).unwrap()
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

pub struct PoolState {
    update_nonce: u16,
    variant:      PoolVariants,
}

impl PoolState {
    pub fn new(variant: PoolVariants) -> Self {
        Self { variant, update_nonce: 0 }
    }

    pub fn increment_state(&mut self, state: PoolUpdate) -> (u16, PoolStateSnapShot) {
        self.update_nonce += 1;
        self.variant.increment_state(state.action, state.logs);
        (self.update_nonce, self.variant.clone().into_snapshot())
    }

    pub fn into_snapshot(&self) -> PoolStateSnapShot {
        self.variant.clone().into_snapshot()
    }

    pub fn address(&self) -> Address {
        match &self.variant {
            PoolVariants::UniswapV2(v) => v.address(),
            PoolVariants::UniswapV3(v) => v.address(),
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
            let log = alloy_primitives::Log::new(log.topics, log.data).unwrap();
            match self {
                PoolVariants::UniswapV3(a) => a.sync_from_log(log).unwrap(),
                PoolVariants::UniswapV2(a) => a.sync_from_log(log).unwrap(),
            }
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
    pub fn get_pair(&self) -> Option<Pair> {
        match &self.action {
            Actions::Swap(s) => Some(Pair(s.token_in, s.token_out)),
            Actions::Mint(m) => Some(Pair(m.token[0], m.token[1])),
            Actions::Burn(b) => Some(Pair(b.token[0], b.token[1])),
            _ => None,
        }
    }
}
