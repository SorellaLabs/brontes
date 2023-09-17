use clickhouse::Row;
use reth_primitives::{Address, H256};
use serde::{Deserialize, Serialize};
use strum::EnumIter;

use crate::{
    normalized_actions::{NormalizedBurn, NormalizedLiquidation, NormalizedMint, NormalizedSwap},
    tree::GasDetails,
};

#[derive(Debug, Serialize, Deserialize, Row)]
pub struct MevBlock {
    pub block_hash: H256,
    pub block_number: u64,
    pub mev_count: u64,
    pub submission_eth_price: u64,
    pub finalized_eth_price: u64,
    /// Gas
    pub cumulative_gas_used: u64,
    pub cumulative_gas_paid: u64,
    pub total_bribe: u64,
    pub cumulative_mev_priority_fee_paid: u64,
    /// Builder address (recipient of coinbase.transfers)
    pub builder_address: Address,
    pub builder_eth_profit: u64,
    pub builder_submission_profit_usd: u64,
    pub builder_finalized_profit_usd: u64,
    /// Proposer address
    pub proposer_fee_recipient: Address,
    pub proposer_mev_reward: u64,
    pub proposer_submission_mev_reward_usd: u64,
    pub proposer_finalized_mev_reward_usd: u64,
    // gas used * (effective gas price - base fee) for all Classified MEV txs
    /// Mev profit
    pub cumulative_mev_submission_profit_usd: u64,
    pub cumulative_mev_finalized_profit_usd: u64,
}

#[derive(Debug, Serialize, Deserialize, Row)]
pub struct ClassifiedMev {
    // can be multiple for sandwich
    pub block_number: u64,
    pub tx_hash: H256,
    pub mev_executor: Address,
    pub mev_collector: Address,
    pub mev_type: MevType,
    pub submission_profit_usd: f64,
    pub finalized_profit_usd: f64,
    pub submission_bribe_usd: f64,
    pub finalized_bribe_usd: f64,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, EnumIter)]
pub enum MevType {
    Sandwich,
    Backrun,
    JitSandwich,
    Jit,
    CexDex,
    Liquidation,
    Unknown,
}

impl Row for MevType {
    const COLUMN_NAMES: &'static [&'static str] = &["mev_type"];
}

pub trait SpecificMev: Serialize + Row {
    const MEV_TYPE: MevType;
    type ComposableResult: SpecificMev;
    type ComposableType: SpecificMev;

    fn bribe(&self) -> u64;
    fn priority_fee_paid(&self) -> u64;
    fn mev_transaction_hashes(&self) -> Vec<H256>;
    fn compose(&mut self, other: Box<Self::ComposableType>) -> Option<Box<Self::ComposableResult>>;
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
    type ComposableResult = JitLiquiditySandwich;
    type ComposableType = JitLiquidity;

    const MEV_TYPE: MevType = MevType::Sandwich;

    fn bribe(&self) -> u64 {
        todo!()
    }

    fn priority_fee_paid(&self) -> u64 {
        todo!()
    }

    fn mev_transaction_hashes(&self) -> Vec<H256> {
        todo!()
    }

    fn compose(&mut self, other: Box<Self::ComposableType>) -> Option<Box<Self::ComposableResult>> {
        todo!()
    }
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

impl SpecificMev for JitLiquiditySandwich {
    type ComposableResult = Self;
    type ComposableType = Self;

    const MEV_TYPE: MevType = MevType::JitSandwich;

    fn bribe(&self) -> u64 {
        todo!()
    }

    fn priority_fee_paid(&self) -> u64 {
        todo!()
    }

    fn mev_transaction_hashes(&self) -> Vec<H256> {
        todo!()
    }

    fn compose(&mut self, other: Box<Self::ComposableType>) -> Option<Box<Self::ComposableResult>> {
        None
    }
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
    type ComposableResult = Self;
    type ComposableType = Self;

    const MEV_TYPE: MevType = MevType::CexDex;

    fn bribe(&self) -> u64 {
        todo!()
    }

    fn priority_fee_paid(&self) -> u64 {
        todo!()
    }

    fn mev_transaction_hashes(&self) -> Vec<H256> {
        todo!()
    }

    fn compose(&mut self, other: Box<Self::ComposableType>) -> Option<Box<Self::ComposableResult>> {
        None
    }
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
    type ComposableResult = Self;
    type ComposableType = Self;

    const MEV_TYPE: MevType = MevType::Liquidation;

    fn bribe(&self) -> u64 {
        todo!()
    }

    fn priority_fee_paid(&self) -> u64 {
        todo!()
    }

    fn mev_transaction_hashes(&self) -> Vec<H256> {
        todo!()
    }

    fn compose(&mut self, other: Box<Self::ComposableType>) -> Option<Box<Self::ComposableResult>> {
        None
    }
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

impl SpecificMev for JitLiquidity {
    type ComposableResult = JitLiquiditySandwich;
    type ComposableType = Sandwich;

    const MEV_TYPE: MevType = MevType::Jit;

    fn bribe(&self) -> u64 {
        todo!()
    }

    fn priority_fee_paid(&self) -> u64 {
        todo!()
    }

    fn mev_transaction_hashes(&self) -> Vec<H256> {
        todo!()
    }

    fn compose(&mut self, other: Box<Self::ComposableType>) -> Option<Box<Self::ComposableResult>> {
        todo!()
    }
}
