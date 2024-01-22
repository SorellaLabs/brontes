use ::serde::ser::{Serialize, SerializeStruct, Serializer};
use sorella_db_databases::clickhouse::{fixed_string::FixedString, DbRow};

use super::normalized_actions::{
    ClickhouseVecNormalizedMintOrBurn, ClickhouseVecNormalizedMintOrBurnWithTxHash,
    ClickhouseVecNormalizedSwap,
};
use crate::{
    classified_mev::JitLiquiditySandwich,
    serde_utils::{
        gas_details::ClickhouseVecGasDetails, normalized_actions::ClickhouseDoubleVecNormalizedSwap,
    },
};

impl Serialize for JitLiquiditySandwich {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ser_struct = serializer.serialize_struct("JitLiquiditySandwich", 34)?;

        // frontruns
        ser_struct.serialize_field(
            "frontrun_tx_hash",
            &FixedString::from(format!("{:?}", self.frontrun_tx_hash.first().unwrap_or_default())),
        )?;

        let frontrun_swaps: ClickhouseDoubleVecNormalizedSwap =
            (self.frontrun_tx_hash.clone(), self.frontrun_swaps.clone()).into();
        ser_struct.serialize_field("frontrun_swaps.tx_hash", &frontrun_swaps.tx_hash)?;
        ser_struct.serialize_field("frontrun_swaps.trace_idx", &frontrun_swaps.trace_index)?;
        ser_struct.serialize_field("frontrun_swaps.from", &frontrun_swaps.from)?;
        ser_struct.serialize_field("frontrun_swaps.recipient", &frontrun_swaps.recipient)?;
        ser_struct.serialize_field("frontrun_swaps.pool", &frontrun_swaps.pool)?;
        ser_struct.serialize_field("frontrun_swaps.token_in", &frontrun_swaps.token_in)?;
        ser_struct.serialize_field("frontrun_swaps.token_out", &frontrun_swaps.token_out)?;
        ser_struct.serialize_field("frontrun_swaps.amount_in", &frontrun_swaps.amount_in)?;
        ser_struct.serialize_field("frontrun_swaps.amount_out", &frontrun_swaps.amount_out)?;

        let frontrun_mints: ClickhouseVecNormalizedMintOrBurnWithTxHash =
            (self.frontrun_tx_hash.clone(), self.frontrun_mints.clone()).into();

        ser_struct.serialize_field("frontrun_mints.tx_hash", &frontrun_mints.tx_hash)?;
        ser_struct.serialize_field("frontrun_mints.trace_idx", &frontrun_mints.trace_index)?;
        ser_struct.serialize_field("frontrun_mints.from", &frontrun_mints.from)?;
        ser_struct.serialize_field("frontrun_mints.to", &frontrun_mints.to)?;
        ser_struct.serialize_field("frontrun_mints.recipient", &frontrun_mints.recipient)?;
        ser_struct.serialize_field("frontrun_mints.tokens", &frontrun_mints.tokens)?;
        ser_struct.serialize_field("frontrun_mints.amounts", &frontrun_mints.amounts)?;

        let frontrun_gas_details: ClickhouseVecGasDetails =
            (self.frontrun_tx_hash.clone(), self.frontrun_gas_details.clone()).into();
        ser_struct
            .serialize_field("frontrun_gas_details.tx_hash", &frontrun_gas_details.tx_hash)?;
        ser_struct.serialize_field(
            "frontrun_gas_details.coinbase_transfer",
            &frontrun_gas_details.coinbase_transfer,
        )?;
        ser_struct.serialize_field(
            "frontrun_gas_details.priority_fee",
            &frontrun_gas_details.priority_fee,
        )?;
        ser_struct
            .serialize_field("frontrun_gas_details.gas_used", &frontrun_gas_details.gas_used)?;
        ser_struct.serialize_field(
            "frontrun_gas_details.effective_gas_price",
            &frontrun_gas_details.effective_gas_price,
        )?;

        // victims
        let victim_swaps: ClickhouseDoubleVecNormalizedSwap =
            (self.victim_swaps_tx_hashes.clone(), self.victim_swaps.clone()).into();
        ser_struct.serialize_field("victim_swaps.tx_hash", &victim_swaps.tx_hash)?;
        ser_struct.serialize_field("victim_swaps.trace_idx", &victim_swaps.trace_index)?;
        ser_struct.serialize_field("victim_swaps.from", &victim_swaps.from)?;
        ser_struct.serialize_field("victim_swaps.recipient", &victim_swaps.recipient)?;
        ser_struct.serialize_field("victim_swaps.pool", &victim_swaps.pool)?;
        ser_struct.serialize_field("victim_swaps.token_in", &victim_swaps.token_in)?;
        ser_struct.serialize_field("victim_swaps.token_out", &victim_swaps.token_out)?;
        ser_struct.serialize_field("victim_swaps.amount_in", &victim_swaps.amount_in)?;
        ser_struct.serialize_field("victim_swaps.amount_out", &victim_swaps.amount_out)?;

        let victim_gas_details: ClickhouseVecGasDetails =
            (self.victim_swaps_tx_hashes.clone(), self.victim_swaps_gas_details.clone()).into();
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

        // backrun
        let fixed_str_backrun_tx_hash = FixedString::from(format!("{:?}", &self.backrun_tx_hash));
        ser_struct.serialize_field("backrun_tx_hash", &fixed_str_backrun_tx_hash)?;

        let backrun_swaps: ClickhouseVecNormalizedSwap = self.backrun_swaps.clone().into();
        let backrun_tx_hash_repeated = vec![&self.backrun_tx_hash]
            .repeat(backrun_swaps.amount_in.len())
            .into_iter()
            .map(|tx| FixedString::from(format!("{:?}", &tx)))
            .collect::<Vec<_>>();

        ser_struct.serialize_field("backrun_swaps.tx_hash", &backrun_tx_hash_repeated)?;
        ser_struct.serialize_field("backrun_swaps.trace_idx", &backrun_swaps.trace_index)?;
        ser_struct.serialize_field("backrun_swaps.from", &backrun_swaps.from)?;
        ser_struct.serialize_field("backrun_swaps.recipient", &backrun_swaps.recipient)?;
        ser_struct.serialize_field("backrun_swaps.pool", &backrun_swaps.pool)?;
        ser_struct.serialize_field("backrun_swaps.token_in", &backrun_swaps.token_in)?;
        ser_struct.serialize_field("backrun_swaps.token_out", &backrun_swaps.token_out)?;
        ser_struct.serialize_field("backrun_swaps.amount_in", &backrun_swaps.amount_in)?;
        ser_struct.serialize_field("backrun_swaps.amount_out", &backrun_swaps.amount_out)?;

        let backrun_burns: ClickhouseVecNormalizedMintOrBurn = self.backrun_burns.clone().into();

        ser_struct.serialize_field("backrun_burns.tx_hash", &backrun_tx_hash_repeated)?;
        ser_struct.serialize_field("backrun_burns.trace_idx", &backrun_burns.trace_index)?;
        ser_struct.serialize_field("backrun_burns.from", &backrun_burns.from)?;
        ser_struct.serialize_field("backrun_burns.to", &backrun_burns.to)?;
        ser_struct.serialize_field("backrun_burns.recipient", &backrun_burns.recipient)?;
        ser_struct.serialize_field("backrun_burns.tokens", &backrun_burns.tokens)?;
        ser_struct.serialize_field("backrun_burns.amounts", &backrun_burns.amounts)?;

        ser_struct
            .serialize_field("backrun_gas_details.tx_hash", &vec![fixed_str_backrun_tx_hash])?;
        ser_struct.serialize_field(
            "backrun_gas_details.coinbase_transfer",
            &vec![self.backrun_gas_details.coinbase_transfer],
        )?;
        ser_struct.serialize_field(
            "backrun_gas_details.priority_fee",
            &vec![self.backrun_gas_details.priority_fee],
        )?;
        ser_struct.serialize_field(
            "backrun_gas_details.gas_used",
            &vec![self.backrun_gas_details.gas_used],
        )?;
        ser_struct.serialize_field(
            "backrun_gas_details.effective_gas_price",
            &vec![self.backrun_gas_details.effective_gas_price],
        )?;

        ser_struct.end()
    }
}

impl DbRow for JitLiquiditySandwich {
    const COLUMN_NAMES: &'static [&'static str] = &[
        "frontrun_tx_hash",
        "frontrun_swaps.tx_hash",
        "frontrun_swaps.trace_idx",
        "frontrun_swaps.from",
        "frontrun_swaps.recipient",
        "frontrun_swaps.pool",
        "frontrun_swaps.token_in",
        "frontrun_swaps.token_out",
        "frontrun_swaps.amount_in",
        "frontrun_swaps.amount_out",
        "frontrun_mints.tx_hash",
        "frontrun_mints.trace_idx",
        "frontrun_mints.from",
        "frontrun_mints.to",
        "frontrun_mints.recipient",
        "frontrun_mints.tokens",
        "frontrun_mints.amounts",
        "frontrun_gas_details.tx_hash",
        "frontrun_gas_details.coinbase_transfer",
        "frontrun_gas_details.priority_fee",
        "frontrun_gas_details.gas_used",
        "frontrun_gas_details.effective_gas_price",
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
        "backrun_tx_hash",
        "backrun_swaps.tx_hash",
        "backrun_swaps.trace_idx",
        "backrun_swaps.from",
        "backrun_swaps.recipient",
        "backrun_swaps.pool",
        "backrun_swaps.token_in",
        "backrun_swaps.token_out",
        "backrun_swaps.amount_in",
        "backrun_swaps.amount_out",
        "backrun_burns.tx_hash",
        "backrun_burns.trace_idx",
        "backrun_burns.from",
        "backrun_burns.to",
        "backrun_burns.recipient",
        "backrun_burns.tokens",
        "backrun_burns.amounts",
        "backrun_gas_details.tx_hash",
        "backrun_gas_details.coinbase_transfer",
        "backrun_gas_details.priority_fee",
        "backrun_gas_details.gas_used",
        "backrun_gas_details.effective_gas_price",
    ];
}
