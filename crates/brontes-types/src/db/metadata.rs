use std::collections::HashSet;

use alloy_primitives::{Address, TxHash, U256};
use clickhouse::Row;
use malachite::{num::basic::traits::Zero, Rational};
use redefined::Redefined;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{self, Serialize};
use serde_with::serde_as;

use super::{builder::BuilderInfo, cex::CexPriceMap, dex::DexQuotes};
use crate::{
    constants::{USDC_ADDRESS, WETH_ADDRESS},
    db::redefined_types::primitives::*,
    implement_table_value_codecs_with_zc,
    pair::Pair,
    serde_utils::{option_addresss, u256, vec_txhash},
};

/// libmdbx type
#[serde_as]
#[derive(
    Debug, Default, Row, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Redefined,
)]
#[redefined_attr(derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    rDeserialize,
    rSerialize,
    Archive
))]
pub struct BlockMetadataInner {
    #[serde(with = "u256")]
    pub block_hash:             U256,
    pub block_timestamp:        u64,
    pub relay_timestamp:        Option<u64>,
    pub p2p_timestamp:          Option<u64>,
    #[serde(with = "option_addresss")]
    pub proposer_fee_recipient: Option<Address>,
    pub proposer_mev_reward:    Option<u128>,
    #[serde(with = "vec_txhash")]
    pub private_flow:           Vec<TxHash>,
}

implement_table_value_codecs_with_zc!(BlockMetadataInnerRedefined);

/// Aggregated Metadata
#[derive(Debug, Clone, derive_more::Deref, derive_more::AsRef, Default)]
pub struct Metadata {
    #[deref]
    #[as_ref]
    pub block_metadata: BlockMetadata,
    pub cex_quotes:     CexPriceMap,
    pub dex_quotes:     Option<DexQuotes>,
    pub builder_info:   Option<BuilderInfo>,
}

impl Metadata {
    pub fn get_gas_price_usd(&self, gas_used: u128) -> Rational {
        let gas_used_rational = Rational::from_unsigneds(gas_used, 10u128.pow(18));
        let eth_price = if self.block_metadata.eth_prices == Rational::ZERO {
            if let Some(dex_quotes) = &self.dex_quotes {
                dex_quotes
                    .price_at_or_before(Pair(WETH_ADDRESS, USDC_ADDRESS), dex_quotes.0.len())
                    .map(|price| price.post_state)
                    .unwrap_or(Rational::ZERO)
            } else {
                Rational::ZERO
            }
        } else {
            self.block_metadata.eth_prices.clone()
        };

        gas_used_rational * eth_price
    }

    pub fn into_full_metadata(mut self, dex_quotes: DexQuotes) -> Self {
        self.dex_quotes = Some(dex_quotes);
        self
    }

    pub fn with_builder_info(mut self, builder_info: BuilderInfo) -> Self {
        self.builder_info = Some(builder_info);
        self
    }

    pub fn block_num(&self) -> u64 {
        self.block_num
    }
}

/// Block Metadata
#[derive(Debug, Clone, Default)]
pub struct BlockMetadata {
    pub block_num:              u64,
    pub block_hash:             U256,
    pub block_timestamp:        u64,
    pub relay_timestamp:        Option<u64>,
    pub p2p_timestamp:          Option<u64>,
    pub proposer_fee_recipient: Option<Address>,
    pub proposer_mev_reward:    Option<u128>,
    /// Best ask at p2p timestamp
    pub eth_prices:             Rational,
    /// Tx
    pub private_flow:           HashSet<TxHash>,
}

impl BlockMetadata {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        block_num: u64,
        block_hash: U256,
        block_timestamp: u64,
        relay_timestamp: Option<u64>,
        p2p_timestamp: Option<u64>,
        proposer_fee_recipient: Option<Address>,
        proposer_mev_reward: Option<u128>,
        eth_prices: Rational,
        private_flow: HashSet<TxHash>,
    ) -> Self {
        Self {
            block_num,
            block_hash,
            relay_timestamp,
            p2p_timestamp,
            eth_prices,
            proposer_fee_recipient,
            proposer_mev_reward,
            private_flow,
            block_timestamp,
        }
    }

    pub fn into_metadata(
        self,
        cex_quotes: CexPriceMap,
        dex_quotes: Option<DexQuotes>,
        builder_info: Option<BuilderInfo>,
    ) -> Metadata {
        Metadata { block_metadata: self, cex_quotes, dex_quotes, builder_info }
    }
}
