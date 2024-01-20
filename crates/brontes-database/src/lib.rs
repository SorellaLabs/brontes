use std::collections::HashSet;

use alloy_primitives::{hex, Address, FixedBytes};
use brontes_pricing::types::DexQuotes;
pub mod cex;

pub use brontes_types::extra_processing::Pair;
use cex::CexPriceMap;
use clickhouse::WETH_ADDRESS;
use malachite::{num::basic::traits::Zero, Rational};
use reth_primitives::{TxHash, U256};

use crate::clickhouse::types::DBTokenPricesDB;
pub mod clickhouse;

#[derive(Debug, Clone, derive_more::Deref, derive_more::AsRef)]
pub struct Metadata {
    #[deref]
    #[as_ref]
    pub db:         MetadataDB,
    pub dex_quotes: DexQuotes,
}
const USDC_ADDRESS: Address =
    Address(FixedBytes::<20>(hex!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")));

impl Metadata {
    pub fn get_gas_price_usd(&self, gas_used: u128) -> Rational {
        let gas_used_rational = Rational::from_unsigneds(gas_used, 10u128.pow(18));
        let eth_price = if self.eth_prices == Rational::ZERO {
            self.dex_quotes
                .price_at_or_before(Pair(WETH_ADDRESS, USDC_ADDRESS), self.dex_quotes.0.len())
                .unwrap()
        } else {
            self.eth_prices.clone()
        };

        gas_used_rational * eth_price
    }
}

#[derive(Debug, Clone, Default)]
pub struct MetadataDB {
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

impl MetadataDB {
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

    pub fn into_finalized_metadata(self, prices: DexQuotes) -> Metadata {
        Metadata { db: self, dex_quotes: prices }
    }
}
