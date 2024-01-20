use alloy_rlp::{Decodable, Encodable};
use brontes_types::{
    classified_mev::{
        AtomicBackrun, CexDex, ClassifiedMev, JitLiquidity, JitLiquiditySandwich, Liquidation,
        MevBlock, MevType, PriceKind, Sandwich, SpecificMev,
    },
    libmdbx::redefined_types::primitives::{
        Redefined_Address, Redefined_FixedBytes, Redefined_Uint,
    },
    normalized_actions::{NormalizedBurn, NormalizedLiquidation, NormalizedMint, NormalizedSwap},
    tree::GasDetails,
};
use bytes::BufMut;
use redefined::{Redefined, RedefinedConvert};
use reth_db::{
    table::{Compress, Decompress},
    DatabaseError,
};
use rkyv::Deserialize;

use crate::types::mev_block::MevBlockWithClassified;

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
pub struct Redefined_MevBlockWithClassified {
    pub block: Redefined_MevBlock,
    pub mev:   Vec<(Redefined_ClassifiedMev, Redefined_SpecificMev)>,
}

impl Encodable for Redefined_MevBlockWithClassified {
    fn encode(&self, out: &mut dyn BufMut) {
        let encoded = rkyv::to_bytes::<_, 256>(self).unwrap();

        out.put_slice(&encoded)
    }
}

impl Decodable for Redefined_MevBlockWithClassified {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let archived: &ArchivedRedefined_MevBlockWithClassified =
            unsafe { rkyv::archived_root::<Self>(buf) };

        let this = archived.deserialize(&mut rkyv::Infallible).unwrap();

        Ok(this)
    }
}

impl Compress for Redefined_MevBlockWithClassified {
    type Compressed = Vec<u8>;

    fn compress_to_buf<B: reth_primitives::bytes::BufMut + AsMut<[u8]>>(self, buf: &mut B) {
        let mut encoded = Vec::new();
        self.encode(&mut encoded);
        let encoded_compressed = zstd::encode_all(&*encoded, 0).unwrap();

        buf.put_slice(&encoded_compressed);
    }
}

impl Decompress for Redefined_MevBlockWithClassified {
    fn decompress<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
        let binding = value.as_ref().to_vec();

        let encoded_decompressed = zstd::decode_all(&*binding).unwrap();
        let buf = &mut encoded_decompressed.as_slice();

        Redefined_MevBlockWithClassified::decode(buf).map_err(|_| DatabaseError::Decode)
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
#[redefined(MevBlock)]
pub struct Redefined_MevBlock {
    pub block_hash: Redefined_FixedBytes<32>,
    pub block_number: u64,
    pub mev_count: u64,
    pub finalized_eth_price: f64,
    pub cumulative_gas_used: u128,
    pub cumulative_gas_paid: u128,
    pub total_bribe: u128,
    pub cumulative_mev_priority_fee_paid: u128,
    pub builder_address: Redefined_Address,
    pub builder_eth_profit: i128,
    pub builder_finalized_profit_usd: f64,
    pub proposer_fee_recipient: Option<Redefined_Address>,
    pub proposer_mev_reward: Option<u128>,
    pub proposer_finalized_profit_usd: Option<f64>,
    pub cumulative_mev_finalized_profit_usd: f64,
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
#[redefined(ClassifiedMev)]
pub struct Redefined_ClassifiedMev {
    pub block_number:         u64,
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
#[redefined(SpecificMev)]
pub enum Redefined_SpecificMev {
    Sandwich(Redefined_Sandwich),
    AtomicBackrun(Redefined_AtomicBackrun),
    JitSandwich(Redefined_JitLiquiditySandwich),
    Jit(Redefined_JitLiquidity),
    CexDex(Redefined_CexDex),
    Liquidation(Redefined_Liquidation),
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
pub struct Redefined_Sandwich {
    pub frontrun_tx_hash:         Redefined_FixedBytes<32>,
    pub frontrun_swaps:           Vec<Redefined_NormalizedSwap>,
    pub frontrun_gas_details:     GasDetails,
    pub victim_swaps_tx_hashes:   Vec<Redefined_FixedBytes<32>>,
    pub victim_swaps:             Vec<Vec<Redefined_NormalizedSwap>>,
    pub victim_swaps_gas_details: Vec<GasDetails>,
    pub backrun_tx_hash:          Redefined_FixedBytes<32>,
    pub backrun_swaps:            Vec<Redefined_NormalizedSwap>,
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
pub struct Redefined_AtomicBackrun {
    pub tx_hash:     Redefined_FixedBytes<32>,
    pub swaps:       Vec<Redefined_NormalizedSwap>,
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
pub struct Redefined_JitLiquiditySandwich {
    pub frontrun_tx_hash:         Redefined_FixedBytes<32>,
    pub frontrun_swaps:           Vec<Redefined_NormalizedSwap>,
    pub frontrun_mints:           Vec<Redefined_NormalizedMint>,
    pub frontrun_gas_details:     GasDetails,
    pub victim_swaps_tx_hashes:   Vec<Redefined_FixedBytes<32>>,
    pub victim_swaps:             Vec<Vec<Redefined_NormalizedSwap>>,
    pub victim_swaps_gas_details: Vec<GasDetails>,
    pub backrun_tx_hash:          Redefined_FixedBytes<32>,
    pub backrun_swaps:            Vec<Redefined_NormalizedSwap>,
    pub backrun_burns:            Vec<Redefined_NormalizedBurn>,
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
pub struct Redefined_JitLiquidity {
    pub frontrun_mint_tx_hash: Redefined_FixedBytes<32>,
    pub frontrun_mints: Vec<Redefined_NormalizedMint>,
    pub frontrun_mint_gas_details: GasDetails,
    pub victim_swaps_tx_hashes: Vec<Redefined_FixedBytes<32>>,
    pub victim_swaps: Vec<Vec<Redefined_NormalizedSwap>>,
    pub victim_swaps_gas_details_tx_hashes: Vec<Redefined_FixedBytes<32>>,
    pub victim_swaps_gas_details: Vec<GasDetails>,
    pub backrun_burn_tx_hash: Redefined_FixedBytes<32>,
    pub backrun_burns: Vec<Redefined_NormalizedBurn>,
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
pub struct Redefined_CexDex {
    pub tx_hash:        Redefined_FixedBytes<32>,
    pub swaps:          Vec<Redefined_NormalizedSwap>,
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
pub struct Redefined_Liquidation {
    pub liquidation_tx_hash: Redefined_FixedBytes<32>,
    pub trigger:             Redefined_FixedBytes<32>,
    pub liquidation_swaps:   Vec<Redefined_NormalizedSwap>,
    pub liquidations:        Vec<Redefined_NormalizedLiquidation>,
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
pub struct Redefined_NormalizedSwap {
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
pub struct Redefined_NormalizedLiquidation {
    pub trace_index:      u64,
    pub pool:             Redefined_Address,
    pub liquidator:       Redefined_Address,
    pub debtor:           Redefined_Address,
    pub collateral_asset: Redefined_Address,
    pub debt_asset:       Redefined_Address,
    pub amount:           Redefined_Uint<256, 4>,
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
pub struct Redefined_NormalizedBurn {
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
pub struct Redefined_NormalizedMint {
    pub trace_index: u64,
    pub from:        Redefined_Address,
    pub to:          Redefined_Address,
    pub recipient:   Redefined_Address,
    pub token:       Vec<Redefined_Address>,
    pub amount:      Vec<Redefined_Uint<256, 4>>,
}
