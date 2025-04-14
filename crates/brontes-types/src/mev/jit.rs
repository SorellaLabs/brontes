use std::fmt::Debug;

use ahash::HashSet;
use clickhouse::DbRow;
use redefined::Redefined;
use alloy_primitives::B256;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{
    ser::{SerializeStruct, Serializer},
    Deserialize, Serialize,
};
use serde_with::serde_as;

use super::{Mev, MevType};
use crate::{
    db::redefined_types::primitives::*, normalized_actions::*, tree::ClickhouseVecGasDetails,
    Protocol,
};
#[allow(unused_imports)]
use crate::{
    display::utils::display_sandwich,
    normalized_actions::{NormalizedBurn, NormalizedLiquidation, NormalizedMint, NormalizedSwap},
    tree::GasDetails,
};

#[serde_as]
#[derive(Debug, Deserialize, PartialEq, Clone, Default, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct JitLiquidity {
    pub frontrun_mint_tx_hash: B256,
    pub block_number: u64,
    pub frontrun_mints: Vec<NormalizedMint>,
    #[redefined(same_fields)]
    pub frontrun_mint_gas_details: GasDetails,
    pub victim_swaps_tx_hashes: Vec<B256>,
    pub victim_swaps: Vec<Vec<NormalizedSwap>>,
    pub victim_swaps_gas_details_tx_hashes: Vec<B256>,
    #[redefined(same_fields)]
    pub victim_swaps_gas_details: Vec<GasDetails>,
    pub backrun_burn_tx_hash: B256,
    pub backrun_burns: Vec<NormalizedBurn>,
    #[redefined(same_fields)]
    pub backrun_burn_gas_details: GasDetails,
}

impl Mev for JitLiquidity {
    fn mev_type(&self) -> MevType {
        MevType::Jit
    }

    fn mev_transaction_hashes(&self) -> Vec<B256> {
        vec![self.frontrun_mint_tx_hash, self.backrun_burn_tx_hash]
    }

    fn total_gas_paid(&self) -> u128 {
        self.frontrun_mint_gas_details.gas_paid() + self.backrun_burn_gas_details.gas_paid()
    }

    fn total_priority_fee_paid(&self, base_fee: u128) -> u128 {
        self.frontrun_mint_gas_details.priority_fee_paid(base_fee)
            + self.backrun_burn_gas_details.priority_fee_paid(base_fee)
    }

    fn bribe(&self) -> u128 {
        self.frontrun_mint_gas_details
            .coinbase_transfer
            .unwrap_or(0)
            + self.backrun_burn_gas_details.coinbase_transfer.unwrap_or(0)
    }

    fn protocols(&self) -> HashSet<Protocol> {
        // Can just do frontrun mints because the extraction is symmetric + victims
        // swaps only interesting as they relate to Jit
        self.frontrun_mints
            .iter()
            .map(|swap| swap.protocol)
            .collect()
    }
}

impl Serialize for JitLiquidity {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ser_struct = serializer.serialize_struct("JitLiquidity", 31)?;

        // frontrun mint
        ser_struct.serialize_field(
            "frontrun_mint_tx_hash",
            &format!("{:?}", self.frontrun_mint_tx_hash),
        )?;
        ser_struct.serialize_field("block_number", &self.block_number)?;

        let frontrun_mints: ClickhouseVecNormalizedMintOrBurn = self
            .frontrun_mints
            .clone()
            .try_into()
            .map_err(serde::ser::Error::custom)?;

        ser_struct.serialize_field("frontrun_mints.trace_idx", &frontrun_mints.trace_index)?;
        ser_struct.serialize_field("frontrun_mints.from", &frontrun_mints.from)?;
        ser_struct.serialize_field("frontrun_mints.pool", &frontrun_mints.pool)?;
        ser_struct.serialize_field("frontrun_mints.recipient", &frontrun_mints.recipient)?;
        ser_struct.serialize_field("frontrun_mints.tokens", &frontrun_mints.tokens)?;
        ser_struct.serialize_field("frontrun_mints.amounts", &frontrun_mints.amounts)?;

        let frontrun_mint_gas_details = (
            self.frontrun_mint_gas_details.coinbase_transfer,
            self.frontrun_mint_gas_details.priority_fee,
            self.frontrun_mint_gas_details.gas_used,
            self.frontrun_mint_gas_details.effective_gas_price,
        );

        ser_struct.serialize_field("frontrun_mint_gas_details", &(frontrun_mint_gas_details))?;

        // victim swaps
        let victim_swaps: ClickhouseDoubleVecNormalizedSwap =
            (self.victim_swaps_tx_hashes.clone(), self.victim_swaps.clone())
                .try_into()
                .map_err(serde::ser::Error::custom)?;
        ser_struct.serialize_field("victim_swaps.tx_hash", &victim_swaps.tx_hash)?;
        ser_struct.serialize_field("victim_swaps.trace_idx", &victim_swaps.trace_index)?;
        ser_struct.serialize_field("victim_swaps.from", &victim_swaps.from)?;
        ser_struct.serialize_field("victim_swaps.recipient", &victim_swaps.recipient)?;
        ser_struct.serialize_field("victim_swaps.pool", &victim_swaps.pool)?;
        ser_struct.serialize_field("victim_swaps.token_in", &victim_swaps.token_in)?;
        ser_struct.serialize_field("victim_swaps.token_out", &victim_swaps.token_out)?;
        ser_struct.serialize_field("victim_swaps.amount_in", &victim_swaps.amount_in)?;
        ser_struct.serialize_field("victim_swaps.amount_out", &victim_swaps.amount_out)?;

        let victim_gas_details: ClickhouseVecGasDetails = (
            self.victim_swaps_gas_details_tx_hashes.clone(),
            self.victim_swaps_gas_details.clone(),
        )
            .into();
        ser_struct.serialize_field("victim_gas_details.tx_hash", &victim_gas_details.tx_hash)?;
        ser_struct.serialize_field(
            "victim_gas_details.coinbase_transfer",
            &victim_gas_details.coinbase_transfer,
        )?;
        ser_struct
            .serialize_field("victim_gas_details.priority_fee", &victim_gas_details.priority_fee)?;
        ser_struct.serialize_field("victim_gas_details.gas_used", &victim_gas_details.gas_used)?;
        ser_struct.serialize_field(
            "victim_gas_details.effective_gas_price",
            &victim_gas_details.effective_gas_price,
        )?;

        // backrun burn
        ser_struct
            .serialize_field("backrun_burn_tx_hash", &format!("{:?}", self.backrun_burn_tx_hash))?;

        let backrun_burns: ClickhouseVecNormalizedMintOrBurn = self
            .backrun_burns
            .clone()
            .try_into()
            .map_err(serde::ser::Error::custom)?;

        ser_struct.serialize_field("backrun_burns.trace_idx", &backrun_burns.trace_index)?;
        ser_struct.serialize_field("backrun_burns.from", &backrun_burns.from)?;
        ser_struct.serialize_field("backrun_burns.pool", &backrun_burns.pool)?;
        ser_struct.serialize_field("backrun_burns.recipient", &backrun_burns.recipient)?;
        ser_struct.serialize_field("backrun_burns.tokens", &backrun_burns.tokens)?;
        ser_struct.serialize_field("backrun_burns.amounts", &backrun_burns.amounts)?;

        let backrun_burn_gas_details = (
            self.backrun_burn_gas_details.coinbase_transfer,
            self.backrun_burn_gas_details.priority_fee,
            self.backrun_burn_gas_details.gas_used,
            self.backrun_burn_gas_details.effective_gas_price,
        );

        ser_struct.serialize_field("backrun_burn_gas_details", &(backrun_burn_gas_details))?;

        ser_struct.end()
    }
}

impl DbRow for JitLiquidity {
    const COLUMN_NAMES: &'static [&'static str] = &[
        "frontrun_mint_tx_hash",
        "block_number",
        "frontrun_mints.trace_idx",
        "frontrun_mints.from",
        "frontrun_mints.pool",
        "frontrun_mints.recipient",
        "frontrun_mints.tokens",
        "frontrun_mints.amounts",
        "frontrun_mint_gas_details",
        "victim_swaps.tx_hash",
        "victim_swaps.trace_idx",
        "victim_swaps.from",
        "victim_swaps.recipient",
        "victim_swaps.pool",
        "victim_swaps.token_in",
        "victim_swaps.token_out",
        "victim_swaps.amount_in",
        "victim_swaps.amount_out",
        "victim_gas_details.tx_hash",
        "victim_gas_details.coinbase_transfer",
        "victim_gas_details.priority_fee",
        "victim_gas_details.gas_used",
        "victim_gas_details.effective_gas_price",
        "backrun_burn_tx_hash",
        "backrun_burns.trace_idx",
        "backrun_burns.from",
        "backrun_burns.pool",
        "backrun_burns.recipient",
        "backrun_burns.tokens",
        "backrun_burns.amounts",
        "backrun_burn_gas_details",
    ];
}
