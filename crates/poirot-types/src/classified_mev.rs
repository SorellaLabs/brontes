use clickhouse::Row;
use malachite::Rational;
use reth_primitives::{Address, H256};
use serde::{Deserialize, Serialize};

use crate::{normalized_actions::NormalizedSwap, tree::GasDetails};

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
    pub mev_bot: Address,
    pub mev_type: String,
    pub submission_profit_usd: f64,
    pub submission_bribe_uds: f64,
    pub finalized_bribe_usd: f64,
}

#[derive(Debug)]

pub enum MevType {
    Sandwich,
    CexDex,
    Liquidation,
    Unknown,
}

#[derive(Debug, Serialize, Row)]
pub struct Sandwich {
    pub front_run: H256,
    pub front_run_gas_details: GasDetails,
    pub front_run_swaps: Vec<NormalizedSwap>,
    pub victim: Vec<H256>,
    pub victim_gas_details: Vec<GasDetails>,
    pub victim_swaps: Vec<NormalizedSwap>,
    pub back_run: H256,
    pub back_run_gas_details: GasDetails,
    pub back_run_swaps: Vec<NormalizedSwap>,
    pub mev_bot: Address,
}

#[derive(Debug, Serialize)]
pub struct CexDex {
    pub swaps: Vec<NormalizedSwap>,
    pub cex_prices: Vec<f64>,
    pub dex_prices: Vec<f64>,
    pub gas_details: Vec<GasDetails>,
}
