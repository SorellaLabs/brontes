use reth_primitives::{Address, H256};
use crate::tree::GasDetails;
use malachite::Rational;
use serde::{Deserialize, Serialize};
use clickhouse::Row;

#[derive(Debug, Serialize, Deserialize, Row)]
pub struct MevBlock {
    block_hash: H256,
    block_number: u64,
    builder_address: Address,
    builder_name: String,
    relays: Vec<String>,
    proposer: Address,
    proposer_rewards: u64,
    cumulative_gas_used: u64,
    cumulative_gas_paid: u64,
    bribe_amount: u64,
    submission_eth_price: u64,
    finalized_eth_price: u64,
    submission_profit_usd: u64,
    finalized_profit_usd: u64,
}

#[derive(Debug, Serialize, Deserialize, Row)]
pub struct ClassifiedMev {
    // can be multiple for sandwich
    pub tx_hash:     Vec<H256>,
    pub block_number: u64,
    pub mev_bot:     Address,
    pub gas_details: Vec<GasDetails>,
    pub tokens:      Vec<Address>,
    pub protocols:   Vec<(String, Address)>,
    // results
    pub submission_profit_usd:        f64,
    pub finalized_profit_usd:         f64,
    pub submission_bribe_uds:         f64,
    pub finalized_bribe_usd:          f64,

}