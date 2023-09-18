use std::any::Any;

use clickhouse::Row;
use reth_primitives::{Address, H256};
use serde::{Deserialize, Serialize};
use strum::EnumIter;

use crate::{
    normalized_actions::{NormalizedBurn, NormalizedLiquidation, NormalizedMint, NormalizedSwap},
    tree::GasDetails
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
    pub cumulative_mev_finalized_profit_usd: u64
}

#[derive(Debug, Serialize, Deserialize, Row)]
pub struct ClassifiedMev {
    // can be multiple for sandwich
    pub block_number:          u64,
    pub tx_hash:               H256,
    pub eoa:                   Address,
    pub mev_contract:          Address,
    pub mev_profit_collector:  Address,
    pub mev_type:              MevType,
    pub submission_profit_usd: f64,
    pub finalized_profit_usd:  f64,
    pub submission_bribe_usd:  f64,
    pub finalized_bribe_usd:   f64
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, EnumIter)]
pub enum MevType {
    Sandwich,
    Backrun,
    JitSandwich,
    Jit,
    CexDex,
    Liquidation,
    Unknown
}

pub enum MevResult {
    Sandwich(Sandwich),
    Jit(JitLiquidity),
    JitSandwich(JitLiquiditySandwich),
    CexDex(CexDex),
    Liquidation(Liquidation)
}

impl Row for MevType {
    const COLUMN_NAMES: &'static [&'static str] = &["mev_type"];
}

/// Because of annoying trait requirements. we do some degenerate shit here.
pub trait SpecificMev {
    fn mev_type(&self) -> MevType;
    fn mev_transaction_hashes(&self) -> Vec<H256>;
}

#[derive(Debug, Serialize, Row)]
pub struct Sandwich {
    pub front_run:             H256,
    pub front_run_gas_details: GasDetails,
    pub front_run_swaps:       Vec<NormalizedSwap>,
    pub victim:                Vec<H256>,
    pub victim_gas_details:    Vec<GasDetails>,
    pub victim_swaps:          Vec<Vec<NormalizedSwap>>,
    pub back_run:              H256,
    pub back_run_gas_details:  GasDetails,
    pub back_run_swaps:        Vec<NormalizedSwap>
}

pub fn compose_sandwich_jit(
    sandwich: Box<dyn Any + 'static>,
    jit: Box<dyn Any + 'static>,
    sandwich_classified: ClassifiedMev,
    jit_classified: ClassifiedMev
) -> (ClassifiedMev, Box<dyn SpecificMev>) {
    let sandwich: Sandwich = *sandwich.downcast().unwrap();
    let jit: JitLiquidity = *jit.downcast().unwrap();

    let jit_sand = Box::new(JitLiquiditySandwich {
        front_run:             sandwich.front_run,
        back_run:              sandwich.back_run,
        front_run_mints:       jit.jit_mints,
        front_run_swaps:       sandwich.front_run_swaps,
        front_run_gas_details: sandwich.front_run_gas_details,
        victim:                sandwich.victim,
        victim_swaps:          sandwich.victim_swaps,
        back_run_burns:        jit.jit_burns,
        back_run_swaps:        sandwich.back_run_swaps,
        victim_gas_details:    sandwich.victim_gas_details,
        back_run_gas_details:  sandwich.back_run_gas_details
    });

    let new_classifed = ClassifiedMev {
        tx_hash:               sandwich.front_run,
        mev_type:              MevType::JitSandwich,
        block_number:          sandwich_classified.block_number,
        eoa:                   jit_classified.eoa,
        mev_contract:          sandwich_classified.mev_contract,
        mev_profit_collector:  sandwich_classified.mev_profit_collector,
        finalized_bribe_usd:   sandwich_classified.finalized_bribe_usd,
        submission_bribe_usd:  sandwich_classified.submission_bribe_usd,
        submission_profit_usd: sandwich_classified.submission_profit_usd
            + jit_classified.submission_profit_usd,
        finalized_profit_usd:  sandwich_classified.finalized_profit_usd
            + jit_classified.finalized_profit_usd
    };

    (new_classifed, jit_sand)
}

impl SpecificMev for Sandwich {
    fn mev_type(&self) -> MevType {
        MevType::Sandwich
    }

    fn mev_transaction_hashes(&self) -> Vec<H256> {
        vec![self.front_run, self.back_run]
    }
}

#[derive(Debug, Serialize, Row)]
pub struct JitLiquiditySandwich {
    pub front_run:             H256,
    pub front_run_gas_details: GasDetails,
    pub front_run_swaps:       Vec<NormalizedSwap>,
    pub front_run_mints:       Vec<NormalizedMint>,
    pub victim:                Vec<H256>,
    pub victim_gas_details:    Vec<GasDetails>,
    pub victim_swaps:          Vec<Vec<NormalizedSwap>>,
    pub back_run:              H256,
    pub back_run_gas_details:  GasDetails,
    pub back_run_burns:        Vec<NormalizedBurn>,
    pub back_run_swaps:        Vec<NormalizedSwap>
}

impl SpecificMev for JitLiquiditySandwich {
    fn mev_type(&self) -> MevType {
        MevType::JitSandwich
    }

    fn mev_transaction_hashes(&self) -> Vec<H256> {
        vec![self.front_run, self.back_run]
    }
}

#[derive(Debug, Serialize, Row)]
pub struct CexDex {
    pub tx_hash:     H256,
    pub swaps:       Vec<NormalizedSwap>,
    pub cex_prices:  Vec<f64>,
    pub dex_prices:  Vec<f64>,
    pub gas_details: Vec<GasDetails>
}

impl SpecificMev for CexDex {
    fn mev_type(&self) -> MevType {
        MevType::CexDex
    }

    fn mev_transaction_hashes(&self) -> Vec<H256> {
        vec![self.tx_hash]
    }
}

#[derive(Debug, Serialize, Row)]
pub struct Liquidation {
    pub trigger:                 H256,
    pub liquidation_tx_hash:     H256,
    pub liquidation_gas_details: GasDetails,
    pub liquidation_swaps:       Vec<NormalizedSwap>,
    pub liquidation:             Vec<NormalizedLiquidation>
}

impl SpecificMev for Liquidation {
    fn mev_type(&self) -> MevType {
        MevType::Liquidation
    }

    fn mev_transaction_hashes(&self) -> Vec<H256> {
        vec![self.liquidation_tx_hash]
    }
}

#[derive(Debug, Serialize, Row)]
pub struct JitLiquidity {
    pub mint_tx_hash:     H256,
    pub mint_gas_details: GasDetails,
    pub jit_mints:        Vec<NormalizedMint>,
    pub swap_tx_hash:     H256,
    pub swap_gas_details: GasDetails,
    pub swaps:            Vec<NormalizedSwap>,
    pub burn_tx_hash:     H256,
    pub burn_gas_details: GasDetails,
    pub jit_burns:        Vec<NormalizedBurn>
}

impl SpecificMev for JitLiquidity {
    fn mev_type(&self) -> MevType {
        MevType::Jit
    }

    fn mev_transaction_hashes(&self) -> Vec<H256> {
        vec![self.mint_tx_hash, self.burn_tx_hash]
    }
}
