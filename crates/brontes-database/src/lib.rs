use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

use malachite::{num::arithmetic::traits::Floor, Rational};
use reth_primitives::{Address, TxHash, U256};

use crate::database::types::DBTokenPricesDB;
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

#[derive(Debug, Clone, Default, Eq)]
pub struct Quote {
    pub timestamp: u64,
    /// Best Ask & Bid price at p2p timestamp (which is when the block is first
    /// propagated by the relay / proposer)
    pub price:     (Rational, Rational),
}

impl Quote {
    pub fn avg(&self) -> Rational {
        (&self.price.0 + &self.price.1) / Rational::from(2)
    }
}

impl PartialEq for Quote {
    fn eq(&self, other: &Self) -> bool {
        self.timestamp == other.timestamp
            && (self.price.0.clone() * Rational::try_from(1000000000).unwrap()).floor()
                == (other.price.0.clone() * Rational::try_from(1000000000).unwrap()).floor()
            && (self.price.1.clone() * Rational::try_from(1000000000).unwrap()).floor()
                == (other.price.1.clone() * Rational::try_from(1000000000).unwrap()).floor()
    }
}

#[derive(Debug, Clone)]

/// There should be 1 entry for how the pair is stored on the CEX and the other
/// order should be the reverse of that
pub struct Quotes(HashMap<Pair, Quote>);

impl Quotes {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn get_quote(&self, pair: Pair) -> Option<&Quote> {
        self.0.get(&pair)
    }
}

impl From<Vec<DBTokenPricesDB>> for Quotes {
    fn from(value: Vec<DBTokenPricesDB>) -> Self {
        let map = value
            .into_iter()
            .map(|token_info| {
                (
                    Pair(
                        Address::from_str(&token_info.key.0).unwrap(),
                        Address::from_str(&token_info.key.1).unwrap(),
                    ),
                    Quote {
                        timestamp: token_info.val.0,
                        price:     (
                            Rational::try_from(token_info.val.1).unwrap(),
                            Rational::try_from(token_info.val.2).unwrap(),
                        ),
                    },
                )
            })
            .collect::<HashMap<Pair, Quote>>();

        Quotes(map)
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

        (&self.eth_prices * &gas_used_rational, &self.eth_prices * gas_used_rational)
    }
}
