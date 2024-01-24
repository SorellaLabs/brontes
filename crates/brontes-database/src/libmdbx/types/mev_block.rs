use brontes_types::{
    classified_mev::{
        AtomicBackrun, BundleData, BundleHeader, CexDex, JitLiquidity, JitLiquiditySandwich,
        Liquidation, MevBlock, MevType, PriceKind, Sandwich,
    },
    db::{
        mev_block::MevBlockWithClassified,
        redefined_types::primitives::{Redefined_Address, Redefined_FixedBytes, Redefined_Uint},
    },
    normalized_actions::{NormalizedBurn, NormalizedLiquidation, NormalizedMint, NormalizedSwap},
    tree::GasDetails,
};
use redefined::{Redefined, RedefinedConvert};
use sorella_db_databases::clickhouse::{self, Row};

use super::{CompressedTable, LibmdbxData};
use crate::libmdbx::MevBlocks;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Row)]
pub struct MevBlocksData {
    pub block_number: u64,
    pub mev_blocks:   MevBlockWithClassified,
}

impl LibmdbxData<MevBlocks> for MevBlocksData {
    fn into_key_val(
        &self,
    ) -> (
        <MevBlocks as reth_db::table::Table>::Key,
        <MevBlocks as CompressedTable>::DecompressedValue,
    ) {
        (self.block_number, self.mev_blocks.clone())
    }
}

#[derive(
    Debug,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Clone,
    Redefined,
)]
#[redefined(MevBlockWithClassified)]
pub struct LibmdbxMevBlockWithClassified {
    pub block: LibmdbxMevBlock,
    pub mev:   Vec<(LibmdbxBundleHeader, LibmdbxBundleData)>,
}

#[derive(
    Debug,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Clone,
    Redefined,
)]
#[redefined(MevBlock)]
pub struct LibmdbxMevBlock {
    pub block_hash: Redefined_FixedBytes<32>,
    pub block_number: u64,
    pub mev_count: u64,
    pub finalized_eth_price: f64,
    pub cumulative_gas_used: u128,
    pub cumulative_gas_paid: u128,
    pub total_bribe: u128,
    pub cumulative_mev_priority_fee_paid: u128,
    pub builder_address: Redefined_Address,
    pub builder_eth_profit: f64,
    pub builder_finalized_profit_usd: f64,
    pub proposer_fee_recipient: Option<Redefined_Address>,
    pub proposer_mev_reward: Option<u128>,
    pub proposer_finalized_profit_usd: Option<f64>,
    pub cumulative_mev_finalized_profit_usd: f64,
    pub possible_missed_arbs: Vec<Redefined_FixedBytes<32>>,
}

#[derive(
    Debug,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Clone,
    Redefined,
)]
#[redefined(BundleHeader)]
pub struct LibmdbxBundleHeader {
    pub block_number:         u64,
    pub mev_tx_index:         u64,
    pub tx_hash:              Redefined_FixedBytes<32>,
    pub eoa:                  Redefined_Address,
    pub mev_contract:         Redefined_Address,
    pub mev_profit_collector: Vec<Redefined_Address>,
    pub finalized_profit_usd: f64,
    pub finalized_bribe_usd:  f64,
    pub mev_type:             MevType,
}

#[derive(
    Debug,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Clone,
    Redefined,
)]
#[redefined(BundleData)]
pub enum LibmdbxBundleData {
    Sandwich(LibmdbxSandwich),
    AtomicBackrun(LibmdbxAtomicBackrun),
    JitSandwich(LibmdbxJitLiquiditySandwich),
    Jit(LibmdbxJitLiquidity),
    CexDex(LibmdbxCexDex),
    Liquidation(LibmdbxLiquidation),
    Unknown,
}

#[derive(
    Debug,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Clone,
    Redefined,
)]
#[redefined(Sandwich)]
pub struct LibmdbxSandwich {
    pub frontrun_tx_hash:         Vec<Redefined_FixedBytes<32>>,
    pub frontrun_swaps:           Vec<Vec<LibmdbxNormalizedSwap>>,
    pub frontrun_gas_details:     Vec<GasDetails>,
    pub victim_swaps_tx_hashes:   Vec<Vec<Redefined_FixedBytes<32>>>,
    pub victim_swaps:             Vec<Vec<LibmdbxNormalizedSwap>>,
    pub victim_swaps_gas_details: Vec<GasDetails>,
    pub backrun_tx_hash:          Redefined_FixedBytes<32>,
    pub backrun_swaps:            Vec<LibmdbxNormalizedSwap>,
    pub backrun_gas_details:      GasDetails,
}

#[derive(
    Debug,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Clone,
    Redefined,
)]
#[redefined(AtomicBackrun)]
pub struct LibmdbxAtomicBackrun {
    pub tx_hash:     Redefined_FixedBytes<32>,
    pub swaps:       Vec<LibmdbxNormalizedSwap>,
    pub gas_details: GasDetails,
}

#[derive(
    Debug,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Clone,
    Redefined,
)]
#[redefined(JitLiquiditySandwich)]
pub struct LibmdbxJitLiquiditySandwich {
    pub frontrun_tx_hash:         Vec<Redefined_FixedBytes<32>>,
    pub frontrun_swaps:           Vec<Vec<LibmdbxNormalizedSwap>>,
    pub frontrun_mints:           Vec<Option<Vec<LibmdbxNormalizedMint>>>,
    pub frontrun_gas_details:     Vec<GasDetails>,
    pub victim_swaps_tx_hashes:   Vec<Vec<Redefined_FixedBytes<32>>>,
    pub victim_swaps:             Vec<Vec<LibmdbxNormalizedSwap>>,
    pub victim_swaps_gas_details: Vec<GasDetails>,
    pub backrun_tx_hash:          Redefined_FixedBytes<32>,
    pub backrun_swaps:            Vec<LibmdbxNormalizedSwap>,
    pub backrun_burns:            Vec<LibmdbxNormalizedBurn>,
    pub backrun_gas_details:      GasDetails,
}

#[derive(
    Debug,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Clone,
    Redefined,
)]
#[redefined(JitLiquidity)]
pub struct LibmdbxJitLiquidity {
    pub frontrun_mint_tx_hash: Redefined_FixedBytes<32>,
    pub frontrun_mints: Vec<LibmdbxNormalizedMint>,
    pub frontrun_mint_gas_details: GasDetails,
    pub victim_swaps_tx_hashes: Vec<Redefined_FixedBytes<32>>,
    pub victim_swaps: Vec<Vec<LibmdbxNormalizedSwap>>,
    pub victim_swaps_gas_details_tx_hashes: Vec<Redefined_FixedBytes<32>>,
    pub victim_swaps_gas_details: Vec<GasDetails>,
    pub backrun_burn_tx_hash: Redefined_FixedBytes<32>,
    pub backrun_burns: Vec<LibmdbxNormalizedBurn>,
    pub backrun_burn_gas_details: GasDetails,
}

#[derive(
    Debug,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Clone,
    Redefined,
)]
#[redefined(CexDex)]
pub struct LibmdbxCexDex {
    pub tx_hash:        Redefined_FixedBytes<32>,
    pub swaps:          Vec<LibmdbxNormalizedSwap>,
    pub gas_details:    GasDetails,
    pub prices_kind:    Vec<PriceKind>,
    pub prices_address: Vec<Redefined_Address>,
    pub prices_price:   Vec<f64>,
}

#[derive(
    Debug,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Clone,
    Redefined,
)]
#[redefined(Liquidation)]
pub struct LibmdbxLiquidation {
    pub liquidation_tx_hash: Redefined_FixedBytes<32>,
    pub trigger:             Redefined_FixedBytes<32>,
    pub liquidation_swaps:   Vec<LibmdbxNormalizedSwap>,
    pub liquidations:        Vec<LibmdbxNormalizedLiquidation>,
    pub gas_details:         GasDetails,
}

#[derive(
    Debug,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Clone,
    Redefined,
)]
#[redefined(NormalizedSwap)]
pub struct LibmdbxNormalizedSwap {
    pub trace_index: u64,
    pub from:        Redefined_Address,
    pub recipient:   Redefined_Address,
    pub pool:        Redefined_Address,
    pub token_in:    Redefined_Address,
    pub token_out:   Redefined_Address,
    pub amount_in:   Redefined_Uint<256, 4>,
    pub amount_out:  Redefined_Uint<256, 4>,
}

#[derive(
    Debug,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Clone,
    Redefined,
)]
#[redefined(NormalizedLiquidation)]
pub struct LibmdbxNormalizedLiquidation {
    pub trace_index:           u64,
    pub pool:                  Redefined_Address,
    pub liquidator:            Redefined_Address,
    pub debtor:                Redefined_Address,
    pub collateral_asset:      Redefined_Address,
    pub debt_asset:            Redefined_Address,
    pub covered_debt:          Redefined_Uint<256, 4>,
    pub liquidated_collateral: Redefined_Uint<256, 4>,
}

#[derive(
    Debug,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Clone,
    Redefined,
)]
#[redefined(NormalizedBurn)]
pub struct LibmdbxNormalizedBurn {
    pub trace_index: u64,
    pub from:        Redefined_Address,
    pub to:          Redefined_Address,
    pub recipient:   Redefined_Address,
    pub token:       Vec<Redefined_Address>,
    pub amount:      Vec<Redefined_Uint<256, 4>>,
}

#[derive(
    Debug,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Clone,
    Redefined,
)]
#[redefined(NormalizedMint)]
pub struct LibmdbxNormalizedMint {
    pub trace_index: u64,
    pub from:        Redefined_Address,
    pub to:          Redefined_Address,
    pub recipient:   Redefined_Address,
    pub token:       Vec<Redefined_Address>,
    pub amount:      Vec<Redefined_Uint<256, 4>>,
}
