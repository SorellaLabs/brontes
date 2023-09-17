use clickhouse::Row;
use reth_primitives::{Address, H256};
use serde::{Deserialize, Serialize};

use crate::{
    normalized_actions::{NormalizedBurn, NormalizedLiquidation, NormalizedMint, NormalizedSwap},
    tree::GasDetails,
};

#[derive(Debug, Serialize, Deserialize, Row)]
pub struct MevBlock {
    block_hash: H256,
    block_number: u64,
    mev_count: u64,
    submission_eth_price: u64,
    finalized_eth_price: u64,
    /// Gas
    cumulative_gas_used: u64,
    cumulative_gas_paid: u64,
    total_bribe: u64,
    cumulative_mev_priority_fee_paid: u64,
    /// Builder address (recipient of coinbase.transfers)
    builder_address: Address,
    builder_eth_profit: u64,
    builder_submission_profit_usd: u64,
    builder_finalized_profit_usd: u64,
    /// Proposer address
    proposer_fee_recipient: Address,
    proposer_mev_reward: u64,
    proposer_submission_mev_reward_usd: u64,
    proposer_finalized_mev_reward_usd: u64,
    // gas used * (effective gas price - base fee) for all Classified MEV txs
    /// Mev profit
    cumulative_mev_submission_profit_usd: u64,
    cumulative_mev_finalized_profit_usd: u64,
}

#[derive(Debug, Serialize, Deserialize, Row)]
pub struct ClassifiedMev {
    // can be multiple for sandwich
    pub block_number: u64,
    pub tx_hash: H256,
    pub mev_bot: Address,
    pub mev_type: MevType,
    pub submission_profit_usd: f64,
    pub finalized_profit_usd: f64,
    pub submission_bribe_usd: f64,
    pub finalized_bribe_usd: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum MevType {
    Sandwich,
    CexDex,
    Liquidation,
    Unknown,
}

impl Row for MevType {
    const COLUMN_NAMES: &'static [&'static str] = &["mev_type"];
}

pub trait SpecificMev: Serialize + Row {
    const MEV_TYPE: MevType;
}

#[derive(Debug, Serialize, Row)]
pub struct Sandwich {
    pub front_run: H256,
    pub front_run_gas_details: GasDetails,
    pub front_run_swaps: Vec<NormalizedSwap>,
    pub victim: Vec<H256>,
    pub victim_gas_details: Vec<GasDetails>,
    pub victim_swaps: Vec<Vec<NormalizedSwap>>,
    pub back_run: H256,
    pub back_run_gas_details: GasDetails,
    pub back_run_swaps: Vec<NormalizedSwap>,
}

impl SpecificMev for Sandwich {
    const MEV_TYPE: MevType = MevType::Sandwich;
}

#[derive(Debug, Serialize, Row)]
pub struct JitLiquiditySandwich {
    pub front_run: H256,
    pub front_run_gas_details: GasDetails,
    pub front_run_swaps: Vec<NormalizedSwap>,
    pub front_run_mint: Vec<NormalizedMint>,
    pub victim: Vec<H256>,
    pub victim_gas_details: Vec<GasDetails>,
    pub victim_swaps: Vec<Vec<NormalizedSwap>>,
    pub back_run: H256,
    pub back_run_gas_details: GasDetails,
    pub back_run_burn: Vec<NormalizedBurn>,
    pub back_run_swaps: Vec<NormalizedSwap>,
}

#[derive(Debug, Serialize, Row)]
pub struct CexDex {
    pub tx_hash: H256,
    pub swaps: Vec<NormalizedSwap>,
    pub cex_prices: Vec<f64>,
    pub dex_prices: Vec<f64>,
    pub gas_details: Vec<GasDetails>,
}

impl SpecificMev for CexDex {
    const MEV_TYPE: MevType = MevType::CexDex;
}

#[derive(Debug, Serialize, Row)]
pub struct Liquidation {
    pub trigger: H256,
    pub liquidation_tx_hash: H256,
    pub liquidation_gas_details: GasDetails,
    pub liquidation_swaps: Vec<NormalizedSwap>,
    pub liquidation: Vec<NormalizedLiquidation>,
}

impl SpecificMev for Liquidation {
    const MEV_TYPE: MevType = MevType::Liquidation;
}

#[derive(Debug, Serialize, Row)]

pub struct JitLiquidity {
    pub mint_tx_hash: H256,
    pub mint_gas_details: GasDetails,
    pub jit_mints: Vec<NormalizedMint>,
    pub swap_tx_hash: H256,
    pub swap_gas_details: GasDetails,
    pub swaps: Vec<NormalizedSwap>,
    pub burn_tx_hash: H256,
    pub burn_gas_details: GasDetails,
    pub jit_burns: Vec<NormalizedBurn>,
}
