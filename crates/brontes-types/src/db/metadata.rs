use alloy_primitives::{Address, BlockHash, TxHash, U256};
use clickhouse::Row;
use malachite::{num::basic::traits::Zero, Rational};
use redefined::Redefined;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::Serialize;
use serde_with::serde_as;

use super::{
    builder::BuilderInfo,
    cex::{quotes::CexPriceMap, trades::CexTradeMap},
    dex::DexQuotes,
    traits::LibmdbxReader,
};
use crate::{
    block_metadata::RelayBlockMetadata,
    constants::WETH_ADDRESS,
    db::{dex::BlockPrice, redefined_types::primitives::*},
    implement_table_value_codecs_with_zc,
    pair::Pair,
    serde_utils::{option_addresss, u256, vec_txhash},
    utils::ToFloatNearest,
    FastHashSet,
};
#[allow(unused_imports)]
use crate::{db::cex::CexExchange, normalized_actions::NormalizedSwap};

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

impl BlockMetadataInner {
    pub fn make_new(
        block_hash: BlockHash,
        block_timestamp: u64,
        relay: Option<RelayBlockMetadata>,
        p2p_timestamp: Option<u64>,
        private_flow: Vec<TxHash>,
    ) -> Self {
        Self {
            block_hash: block_hash.into(),
            block_timestamp,
            relay_timestamp: relay.as_ref().and_then(|r| r.relay_timestamp),
            p2p_timestamp,
            proposer_fee_recipient: relay.as_ref().map(|r| r.proposer_fee_recipient),
            proposer_mev_reward: relay.as_ref().map(|r| r.proposer_mev_reward),
            private_flow,
        }
    }
}

/// Aggregated Metadata
#[derive(Debug, Clone, derive_more::Deref, derive_more::AsRef, Default)]
pub struct Metadata {
    #[deref]
    #[as_ref]
    pub block_metadata: BlockMetadata,
    pub cex_quotes:     CexPriceMap,
    pub dex_quotes:     Option<DexQuotes>,
    pub builder_info:   Option<BuilderInfo>,
    pub cex_trades:     Option<CexTradeMap>,
}

impl Metadata {
    pub fn display_pairs_quotes<DB: LibmdbxReader>(&self, db: &DB) {
        self.cex_quotes.quotes.iter().for_each(|(exchange, pairs)| {
            pairs.keys().for_each(|key| {
                let Ok(token0) = db.try_fetch_token_info(key.0).map(|s| s.symbol.clone()) else {
                    return;
                };
                let Ok(token1) = db.try_fetch_token_info(key.1).map(|s| s.symbol.clone()) else {
                    return;
                };
                if &token0 == "WETH" && &token1 == "USDT" {
                    tracing::info!(?exchange, "{}-{} in quotes", token0, token1);
                }
            });
        });
    }

    pub fn get_gas_price_usd(&self, gas_used: u128, quote_token: Address) -> Rational {
        let gas_used_rational = Rational::from_unsigneds(gas_used, 10u128.pow(18));
        let eth_price = self.get_eth_price(quote_token);

        println!("gas used: {}", gas_used);
        println!("eth price: {}", eth_price.clone().to_float());

        gas_used_rational * eth_price
    }

    /// Retrieves the ETH price in terms of the given quote token.
    ///
    /// First checks the block metadata for a pre-stored price. If that's zero,
    /// falls back to DEX quotes using the average block price.
    pub fn get_eth_price(&self, quote_token: Address) -> Rational {
        if self.block_metadata.eth_prices != Rational::ZERO {
            return self.block_metadata.eth_prices.clone();
        }

        self.dex_quotes
            .as_ref()
            .and_then(|dex_quotes| {
                dex_quotes.price_for_block(Pair(WETH_ADDRESS, quote_token), BlockPrice::Average)
            })
            .unwrap_or(Rational::ZERO)
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
    pub private_flow:           FastHashSet<TxHash>,
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
        private_flow: FastHashSet<TxHash>,
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

    pub fn microseconds_block_timestamp(&self) -> u64 {
        self.block_timestamp * 1_000_000
    }

    pub fn into_metadata(
        self,
        cex_quotes: CexPriceMap,
        dex_quotes: Option<DexQuotes>,
        builder_info: Option<BuilderInfo>,
        cex_trades: Option<CexTradeMap>,
    ) -> Metadata {
        Metadata { block_metadata: self, cex_quotes, dex_quotes, builder_info, cex_trades }
    }
}
