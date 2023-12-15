use std::{
    collections::{HashMap, HashSet},
    ops::MulAssign,
    str::FromStr,
};

use database::types::PoolReservesDB;
use graph::PriceGraph;
use malachite::{
    num::{
        arithmetic::traits::{Floor, Reciprocal, ReciprocalAssign},
        basic::traits::Zero,
    },
    Rational,
};
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
    pub proposer_mev_reward:    u64,
    pub cex_quotes:             PriceGraph<CexQuote>,
    pub dex_quotes:             PriceGraph<DexQuote>,
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

pub trait Quote: MulAssign<Self> + std::fmt::Debug + Clone + Send + Sync + 'static {
    fn inverse_price(&mut self);
}

#[derive(Debug, Clone, Default)]
pub struct DexQuote(HashMap<Address, Rational>);

impl Quote for DexQuote {
    fn inverse_price(&mut self) {
        for v in self.0.values_mut() {
            v.reciprocal_assign()
        }
    }
}

impl MulAssign for DexQuote {
    fn mul_assign(&mut self, rhs: Self) {
        assert!(self.0.len() == rhs.0.len(), "rhs.len() != lhs.len()");

        for (k, v) in rhs.0 {
            *self.0.get_mut(&k).unwrap() *= v;
        }
    }
}

impl From<Vec<PoolReservesDB>> for Quotes<DexQuote> {
    fn from(value: Vec<PoolReservesDB>) -> Self {
        value
            .into_iter()
            .map(|pool_reserve| {
                let pair_w_price = pool_reserve
                    .prices_base_addr
                    .into_iter()
                    .zip(pool_reserve.prices_quote_addr.into_iter())
                    .zip(pool_reserve.prices_price.into_iter())
                    .map(|((base, quote), price)| {
                        (
                            Pair(
                                Address::from_str(&base.to_string()).unwrap(),
                                Address::from_str(&quote.to_string()).unwrap(),
                            ),
                            Rational::try_from(price).unwrap(),
                        )
                    });
                (Address::from_str(&pool_reserve.post_tx_hash.to_string()).unwrap(), pair_w_price)
            })
            .fold(Quotes(HashMap::new()), |mut map, (post_tx, pair_w_price)| {
                for (pair, price) in pair_w_price {
                    assert!(map
                        .0
                        .entry(pair)
                        .or_default()
                        .0
                        .insert(post_tx, price)
                        .is_none());
                }

                map
            })
    }
}

#[derive(Debug, Clone, Hash, Eq, Default)]
pub struct CexQuote {
    pub timestamp: u64,
    /// Best Ask & Bid price at p2p timestamp (which is when the block is first
    /// propagated by the relay / proposer)
    pub price:     (Rational, Rational),
}

impl Quote for CexQuote {
    fn inverse_price(&mut self) {
        self.price.0.reciprocal_assign();
        self.price.1.reciprocal_assign();
    }
}

impl CexQuote {
    pub fn avg(&self) -> Rational {
        (&self.price.0 + &self.price.1) / Rational::from(2)
    }

    pub fn best_ask(&self) -> Rational {
        self.price.0.clone()
    }

    pub fn best_bid(&self) -> Rational {
        self.price.1.clone()
    }
}

impl PartialEq for CexQuote {
    fn eq(&self, other: &Self) -> bool {
        self.timestamp == other.timestamp
            && (self.price.0.clone() * Rational::try_from(1000000000).unwrap()).floor()
                == (other.price.0.clone() * Rational::try_from(1000000000).unwrap()).floor()
            && (self.price.1.clone() * Rational::try_from(1000000000).unwrap()).floor()
                == (other.price.1.clone() * Rational::try_from(1000000000).unwrap()).floor()
    }
}

impl MulAssign for CexQuote {
    fn mul_assign(&mut self, rhs: Self) {
        self.price.0 *= rhs.price.0;
        self.price.1 *= rhs.price.1;
    }
}

#[derive(Debug, Clone)]
/// There should be 1 entry for how the pair is stored on the CEX and the other
/// order should be the reverse of that
pub struct Quotes<Q: Quote>(HashMap<Pair, Q>);

impl<Q: Quote> Quotes<Q> {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn get_quote(&self, pair: &Pair) -> Option<&Q> {
        self.0.get(pair)
    }
}

impl From<Vec<DBTokenPricesDB>> for Quotes<CexQuote> {
    fn from(value: Vec<DBTokenPricesDB>) -> Self {
        let map = value
            .into_iter()
            .map(|token_info| {
                (
                    Pair(
                        Address::from_str(&token_info.key.0).unwrap(),
                        Address::from_str(&token_info.key.1).unwrap(),
                    ),
                    CexQuote {
                        timestamp: token_info.val.0,
                        price:     (
                            Rational::try_from(token_info.val.1).unwrap(),
                            Rational::try_from(token_info.val.2).unwrap(),
                        ),
                    },
                )
            })
            .collect::<HashMap<Pair, CexQuote>>();

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
        cex_quotes: PriceGraph<CexQuote>,
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
    pub fn get_gas_price_usd(&self, gas_used: u64) -> Rational {
        let gas_used_rational = Rational::from_unsigneds(gas_used, 10u64.pow(18));

        &self.eth_prices * gas_used_rational
    }
}
