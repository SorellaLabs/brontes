use std::collections::{HashMap, HashSet};

use malachite::Rational;
use reth_primitives::{Address, TxHash, U256};

pub mod database;

#[derive(Debug, Clone)]
pub struct Metadata {
    pub block_num:              u64,
    pub block_hash:             U256,
    pub relay_timestamp:        u64,
    pub p2p_timestamp:          u64,
    pub proposer_fee_recipient: Address,
    pub proposer_mev_reward:    u64,
    pub cex_quotes:             Quotes,
    /// Best ask at p2p timestamp
    pub eth_prices:             Rational,
    pub mempool_flow:           HashSet<TxHash>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Pair(Address, Address);

#[derive(Debug, Clone)]
pub struct Quote {
    pub timestamp: u64,
    /// Best Ask & Bid price at p2p timestamp (which is when the block is first
    /// propagated by the relay / proposer)
    pub price:     (Rational, Rational),
}
#[derive(Debug, Clone)]

/// There should be 1 entry for how the pair is stored on the CEX and the other
/// order should be the reverse of that
pub struct Quotes(HashMap<Pair, Quote>);

impl Quotes {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn get_quote(&self, pair: Pair) -> Option<&Vec<Quote>> {
        self.0.get(&pair)
    }
}

impl Trades {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn get_trades(&self, pair: Pair) -> Option<&Vec<Trade>> {
        self.0.get(&pair)
    }
}

impl Metadata {
    pub fn new(
        block_num: u64,
        block_hash: U256,
        relay_timestamp: u64,
        p2p_timestamp: u64,
        proposer_fee_recipient: Address,
        proposer_mev_reward: u64,
        cex_quotes: Quotes,
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
        }
    }
}

impl Metadata {
    pub fn get_gas_price_usd(&self, gas_used: u64) -> (Rational, Rational) {
        let gas_used_rational = Rational::from_unsigneds(gas_used, 10u64.pow(18));

        (&self.eth_prices.0 * &gas_used_rational, &self.eth_prices.1 * gas_used_rational)
    }
}
