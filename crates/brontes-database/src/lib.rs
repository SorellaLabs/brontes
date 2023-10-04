use std::collections::{HashMap, HashSet};

use malachite::Rational;
use reth_primitives::{Address, TxHash, U256};

pub mod database;

#[derive(Debug)]
pub struct Metadata {
    pub block_num:              u64,
    pub block_hash:             U256,
    pub relay_timestamp:        u64,
    pub p2p_timestamp:          u64,
    pub proposer_fee_recipient: Address,
    pub proposer_mev_reward:    u64,
    pub token_prices:           HashMap<Address, (Rational, Rational)>,
    pub eth_prices:             (Rational, Rational),
    pub mempool_flow:           HashSet<TxHash>,
}

impl Metadata {
    pub fn new(
        block_num: u64,
        block_hash: U256,
        relay_timestamp: u64,
        p2p_timestamp: u64,
        proposer_fee_recipient: Address,
        proposer_mev_reward: u64,
        token_prices: HashMap<Address, (Rational, Rational)>,
        eth_prices: (Rational, Rational),
        mempool_flow: HashSet<TxHash>,
    ) -> Self {
        Self {
            block_num,
            block_hash,
            relay_timestamp,
            p2p_timestamp,
            token_prices,
            eth_prices,
            proposer_fee_recipient,
            proposer_mev_reward,
            mempool_flow,
        }
    }
}
