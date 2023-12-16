use std::{
    collections::{HashMap, HashSet},
    ops::MulAssign,
    str::FromStr,
};

use graph::PriceGraph;
use malachite::{
    num::{arithmetic::traits::Floor, basic::traits::Zero},
    Rational,
};
use reth_primitives::{Address, TxHash, U256};

use crate::clickhouse::types::DBTokenPricesDB;
pub mod clickhouse;

pub mod graph;

#[derive(Debug, Clone)]
pub struct Metadata {
    pub block_num:              u64,
    pub block_hash:             U256,
    pub relay_timestamp:        u64,
    pub p2p_timestamp:          u64,
    pub proposer_fee_recipient: Address,
    pub proposer_mev_reward:    u128,
    pub cex_quotes:             PriceGraph,
    /// Best ask at p2p timestamp
    pub eth_prices:             Rational,
    pub mempool_flow:           HashSet<TxHash>,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Pair(pub Address, pub Address);

impl Pair {
    pub fn has_base_edge(&self, addr: Address) -> bool {
        self.0 == addr
    }

    pub fn has_quote_edge(&self, addr: Address) -> bool {
        self.1 == addr
    }
}

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

    pub fn best_ask(&self) -> Rational {
        self.price.0.clone()
    }

    pub fn best_bid(&self) -> Rational {
        self.price.1.clone()
    }

    /// inverses the prices
    pub fn inverse_price(&mut self) {
        let (num, denom) = self.price.0.numerator_and_denominator_ref();
        self.price.0 = Rational::from_naturals_ref(denom, num);

        let (num, denom) = self.price.1.numerator_and_denominator_ref();
        self.price.1 = Rational::from_naturals_ref(denom, num);
    }

    pub fn is_default(&self) -> bool {
        self.timestamp == 0 && self.price.0 == Rational::ZERO && self.price.1 == Rational::ZERO
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

impl MulAssign<Quote> for Quote {
    fn mul_assign(&mut self, rhs: Quote) {
        self.price.0 *= rhs.price.0;
        self.price.1 *= rhs.price.1;
    }
}

#[derive(Debug, Clone)]
/// There should be 1 entry for how the pair is stored on the CEX and the other
/// order should be the reverse of that
pub struct QuotesMap(HashMap<Pair, Quote>);

impl QuotesMap {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn wrap(map: HashMap<Pair, Quote>) -> Self {
        Self(map)
    }

    pub fn get_quote(&self, pair: &Pair) -> Option<&Quote> {
        self.0.get(pair)
    }
}

impl From<Vec<DBTokenPricesDB>> for QuotesMap {
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

        QuotesMap(map)
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
        cex_quotes: PriceGraph,
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
    pub fn get_gas_price_usd(&self, gas_used: u128) -> Rational {
        let gas_used_rational = Rational::from_unsigneds(gas_used, 10u128.pow(18));

        &self.eth_prices * gas_used_rational
    }
}
