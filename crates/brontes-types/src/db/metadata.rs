use std::collections::HashSet;

use alloy_primitives::{Address, TxHash, U256};
use malachite::{num::basic::traits::Zero, Rational};
use serde_with::{serde_as, DisplayFromStr};
use sorella_db_databases::{clickhouse, clickhouse::Row};

use super::{cex::CexPriceMap, dex::DexQuotes};
use crate::{
    constants::{USDC_ADDRESS, WETH_ADDRESS},
    pair::Pair,
    serde_primitives::{option_address, u256},
};

/// libmdbx type
#[serde_as]
#[derive(Debug, Default, Row, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MetadataInner {
    #[serde(with = "u256")]
    pub block_hash:             U256,
    pub block_timestamp:        u64,
    pub relay_timestamp:        Option<u64>,
    pub p2p_timestamp:          Option<u64>,
    #[serde(with = "option_address")]
    pub proposer_fee_recipient: Option<Address>,
    pub proposer_mev_reward:    Option<u128>,
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub mempool_flow:           Vec<TxHash>,
}

/// aggregated metadata from clickhouse WITH dex pricing
#[derive(Debug, Clone, derive_more::Deref, derive_more::AsRef)]
pub struct MetadataCombined {
    #[deref]
    #[as_ref]
    pub db:         MetadataNoDex,
    pub dex_quotes: DexQuotes,
}

impl MetadataCombined {
    pub fn get_gas_price_usd(&self, gas_used: u128) -> Rational {
        let gas_used_rational = Rational::from_unsigneds(gas_used, 10u128.pow(18));
        let eth_price = if self.eth_prices == Rational::ZERO {
            self.dex_quotes
                .price_at_or_before(Pair(WETH_ADDRESS, USDC_ADDRESS), self.dex_quotes.0.len())
                .map(|price| price.post_state)
                .unwrap()
        } else {
            self.eth_prices.clone()
        };

        gas_used_rational * eth_price
    }
}

/// aggregated metadata from clickhouse WITHOUT dex pricing
#[derive(Debug, Clone, Default)]
pub struct MetadataNoDex {
    pub block_num:              u64,
    pub block_hash:             U256,
    pub block_timestamp:        u64,
    pub relay_timestamp:        Option<u64>,
    pub p2p_timestamp:          Option<u64>,
    pub proposer_fee_recipient: Option<Address>,
    pub proposer_mev_reward:    Option<u128>,
    pub cex_quotes:             CexPriceMap,
    /// Best ask at p2p timestamp
    pub eth_prices:             Rational,
    pub mempool_flow:           HashSet<TxHash>,
}

impl MetadataNoDex {
    pub fn new(
        block_num: u64,
        block_hash: U256,
        block_timestamp: u64,
        relay_timestamp: Option<u64>,
        p2p_timestamp: Option<u64>,
        proposer_fee_recipient: Option<Address>,
        proposer_mev_reward: Option<u128>,
        cex_quotes: CexPriceMap,
        eth_prices: Rational,
        mempool_flow: HashSet<TxHash>,
    ) -> Self {
        Self {
            block_num,
            block_hash,
            relay_timestamp,
            p2p_timestamp,
            cex_quotes,
            eth_prices,
            proposer_fee_recipient,
            proposer_mev_reward,
            mempool_flow,
            block_timestamp,
        }
    }

    pub fn into_finalized_metadata(self, prices: DexQuotes) -> MetadataCombined {
        MetadataCombined { db: self, dex_quotes: prices }
    }
}
