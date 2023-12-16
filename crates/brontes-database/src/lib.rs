use std::{
    collections::{HashMap, HashSet},
    ops::MulAssign,
};
pub mod cex;
pub mod dex;

pub use brontes_types::extra_processing::Pair;
use cex::{CexPriceMap, CexQuote};
pub use dex::DexQuote;
use graph::PriceGraph;
use malachite::Rational;
use reth_primitives::{Address, TxHash, U256};

use crate::database::types::DBTokenPricesDB;
pub mod database;
pub mod graph;

#[derive(Debug, Clone)]
pub struct Metadata {
    pub block_num:              u64,
    pub block_hash:             U256,
    pub relay_timestamp:        u64,
    pub p2p_timestamp:          u64,
    pub proposer_fee_recipient: Address,
    pub proposer_mev_reward:    u128,
    pub cex_quotes:             CexPriceMap,
    pub dex_quotes:             PriceGraph<DexQuote>,
    /// Best ask at p2p timestamp
    pub eth_prices:             Rational,
    pub mempool_flow:           HashSet<TxHash>,
}

pub trait Quote: MulAssign<Self> + std::fmt::Debug + Clone + Send + Sync + 'static {
    fn inverse_price(&mut self);
}

#[derive(Debug, Clone)]
pub struct DexQuotesMap<Q: Quote>(HashMap<Pair, Q>);

impl<Q: Quote> DexQuotesMap<Q> {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn wrap(map: HashMap<Pair, Q>) -> Self {
        Self(map)
    }

    pub fn get_quote(&self, pair: &Pair) -> Option<&Q> {
        self.0.get(pair)
    }
}

impl Metadata {
    pub fn new(
        block_num: u64,
        block_hash: U256,
        relay_timestamp: u64,
        p2p_timestamp: u64,
        proposer_fee_recipient: Address,
        proposer_mev_reward: u128,
        cex_quotes: CexPriceMap,
        dex_quotes: PriceGraph<DexQuote>,
        eth_prices: Rational,
        mempool_flow: HashSet<TxHash>,
    ) -> Self {
        Self {
            block_num,
            block_hash,
            relay_timestamp,
            p2p_timestamp,
            cex_quotes,
            dex_quotes,
            eth_prices,
            proposer_fee_recipient,
            proposer_mev_reward,
            mempool_flow,
        }
    }
}

impl Metadata {
    pub fn get_gas_price_usd(&self, gas_used: u128) -> Rational {
        let gas_used_rational = Rational::from_unsigneds(gas_used, 10u128.pow(18));

        &self.eth_prices * gas_used_rational
    }
}
