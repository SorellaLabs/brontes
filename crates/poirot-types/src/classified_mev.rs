use clickhouse::Row;
use malachite::Rational;
use reth_primitives::{Address, H256};
use serde::{Deserialize, Serialize};

use crate::tree::GasDetails;

#[derive(Debug, Serialize, Deserialize, Row)]
pub struct MevBlock {
    block_hash: H256,
    block_number: u64,
    builder_address: Address,
    proposer: Address,
    proposer_rewards: u64,
    mev_count: u64,
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
    pub block_number: u64,
    pub tx_hash: H256,
    pub mev_type: String,
}

#[derive(Debug)]

pub enum MevType {
    Sandwich,
    AtomicBackrun,
    CexDex,
    DexDex,
    DexCex,
    Dex,
    Cex,
    Unknown,
}

#[derive(Debug, Serialize, Row)]
pub struct Sandwich<A>
where
    A: Row + Serialize,
{
    pub front_run: (H256, GasDetails),
    pub victim: Vec<H256>,
    pub back_run: H256,
    pub mev_bot: Address,
    pub gas_details: Vec<GasDetails>,
    pub tokens: Vec<Address>,
    pub contracts: Vec<Address>,
    pub action: Vec<A>,
    // results
    pub submission_profit_usd: f64,
    pub finalized_profit_usd: f64,
    pub submission_bribe_uds: f64,
    pub finalized_bribe_usd: f64,
}

#[derive(Debug, Serialize)]
pub struct CexDex {
    pub mev_bot: Address,
    pub gas_details: Vec<GasDetails>,
    pub tokens: Vec<Address>,
    pub contracts: Vec<Address>,
    // results
    pub submission_profit_usd: f64,
    pub finalized_profit_usd: f64,
    pub submission_bribe_uds: f64,
    pub finalized_bribe_usd: f64,
}
