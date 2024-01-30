use brontes_types::{
    db::{
        mev_block::MevBlockWithClassified,
        redefined_types::{
            malachite::Redefined_Rational,
            primitives::{Redefined_Address, Redefined_FixedBytes},
        },
        token_info::{TokenInfo, TokenInfoWithAddress},
    },
    mev::{
        AtomicBackrun, Bundle, BundleData, BundleHeader, CexDex, JitLiquidity,
        JitLiquiditySandwich, Liquidation, MevBlock, MevCount, MevType, PossibleMev,
        PossibleMevCollection, PossibleMevTriggers, Sandwich, TokenProfit, TokenProfits,
    },
    normalized_actions::{NormalizedBurn, NormalizedLiquidation, NormalizedMint, NormalizedSwap},
    GasDetails, PriceKind, Protocol,
};
use redefined::{Redefined, RedefinedConvert};
use sorella_db_databases::clickhouse::{self, Row};

use super::{LibmdbxData, ReturnKV};
use crate::libmdbx::MevBlocks;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Row)]
pub struct MevBlocksData {
    pub block_number: u64,
    pub mev_blocks:   MevBlockWithClassified,
}

impl LibmdbxData<MevBlocks> for MevBlocksData {
    fn into_key_val(&self) -> ReturnKV<MevBlocks> {
        (self.block_number, self.mev_blocks.clone()).into()
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
    pub mev:   Vec<LibmdbxBundle>,
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
    pub mev_count: LibmdbxMevCount,
    pub eth_price: f64,
    pub cumulative_gas_used: u128,
    // Sum of all priority fees in the block
    pub cumulative_priority_fee: u128,
    pub total_bribe: u128,
    pub cumulative_mev_priority_fee_paid: u128,
    pub builder_address: Redefined_Address,
    pub builder_eth_profit: f64,
    pub builder_profit_usd: f64,
    pub proposer_fee_recipient: Option<Redefined_Address>,
    pub proposer_mev_reward: Option<u128>,
    pub proposer_profit_usd: Option<f64>,
    pub cumulative_mev_profit_usd: f64,
    pub possible_mev: LibmdbxPossibleMevCollection,
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
#[redefined(MevCount)]
pub struct LibmdbxMevCount {
    pub mev_count:            u64,
    pub sandwich_count:       Option<u64>,
    pub cex_dex_count:        Option<u64>,
    pub jit_count:            Option<u64>,
    pub jit_sandwich_count:   Option<u64>,
    pub atomic_backrun_count: Option<u64>,
    pub liquidation_count:    Option<u64>,
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
#[redefined(PossibleMevCollection)]
pub struct LibmdbxPossibleMevCollection(pub Vec<LibmdbxPossibleMev>);

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
#[redefined(PossibleMev)]
pub struct LibmdbxPossibleMev {
    pub tx_hash:     Redefined_FixedBytes<32>,
    pub tx_idx:      u64,
    pub gas_details: GasDetails,
    pub triggers:    LibmdbxPossibleMevTriggers,
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
#[redefined(PossibleMevTriggers)]
pub struct LibmdbxPossibleMevTriggers {
    pub is_private:        bool,
    pub coinbase_transfer: bool,
    pub high_priority_fee: bool,
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
    pub tx_index:             u64,
    pub tx_hash:              Redefined_FixedBytes<32>,
    pub eoa:                  Redefined_Address,
    pub mev_contract:         Redefined_Address,
    pub mev_profit_collector: Vec<Redefined_Address>,
    pub profit_usd:           f64,
    pub token_profits:        LibmdbxTokenProfits,
    pub bribe_usd:            f64,
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
#[redefined(Bundle)]
pub struct LibmdbxBundle {
    pub header: LibmdbxBundleHeader,
    pub data:   LibmdbxBundleData,
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
#[redefined(TokenProfit)]
pub struct LibmdbxTokenProfit {
    pub profit_collector: Redefined_Address,
    pub token:            Redefined_Address,
    pub amount:           f64,
    pub usd_value:        f64,
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
#[redefined(TokenProfits)]
pub struct LibmdbxTokenProfits {
    pub profits: Vec<LibmdbxTokenProfit>,
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
#[redefined(TokenInfoWithAddress)]
pub struct LibmdbxTokenInfoWithAddress {
    pub inner:   TokenInfo,
    pub address: Redefined_Address,
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
    pub protocol:    Protocol,
    pub trace_index: u64,
    pub from:        Redefined_Address,
    pub recipient:   Redefined_Address,
    pub pool:        Redefined_Address,
    pub token_in:    LibmdbxTokenInfoWithAddress,
    pub token_out:   LibmdbxTokenInfoWithAddress,
    pub amount_in:   Redefined_Rational,
    pub amount_out:  Redefined_Rational,
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
    pub protocol:              Protocol,
    pub trace_index:           u64,
    pub pool:                  Redefined_Address,
    pub liquidator:            Redefined_Address,
    pub debtor:                Redefined_Address,
    pub collateral_asset:      LibmdbxTokenInfoWithAddress,
    pub debt_asset:            LibmdbxTokenInfoWithAddress,
    pub covered_debt:          Redefined_Rational,
    pub liquidated_collateral: Redefined_Rational,
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
    pub protocol:    Protocol,
    pub trace_index: u64,
    pub from:        Redefined_Address,
    pub to:          Redefined_Address,
    pub recipient:   Redefined_Address,
    pub token:       Vec<LibmdbxTokenInfoWithAddress>,
    pub amount:      Vec<Redefined_Rational>,
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
    pub protocol:    Protocol,
    pub trace_index: u64,
    pub from:        Redefined_Address,
    pub to:          Redefined_Address,
    pub recipient:   Redefined_Address,
    pub token:       Vec<LibmdbxTokenInfoWithAddress>,
    pub amount:      Vec<Redefined_Rational>,
}
