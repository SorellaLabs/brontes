use ::serde::ser::{Serialize, SerializeStruct, Serializer};
use sorella_db_databases::clickhouse::{fixed_string::FixedString, DbRow};

use crate::{
    classified_mev::Sandwich,
    serde_utils::{
        gas_details::ClickhouseVecGasDetails,
        normalized_actions::{ClickhouseDoubleVecNormalizedSwap, ClickhouseVecNormalizedSwap},
    },
};

impl Serialize for Sandwich {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ser_struct = serializer.serialize_struct("Sandwich", 34)?;

        // frontrun
        ser_struct.serialize_field(
            "frontrun_tx_hash",
            &FixedString::from(format!("{:?}", self.frontrun_tx_hash)),
        )?;

        let frontrun_swaps: ClickhouseVecNormalizedSwap = self.frontrun_swaps.clone().into();

        ser_struct.serialize_field("frontrun_swaps.trace_idx", &frontrun_swaps.trace_index)?;
        ser_struct.serialize_field("frontrun_swaps.from", &frontrun_swaps.from)?;
        ser_struct.serialize_field("frontrun_swaps.recipient", &frontrun_swaps.recipient)?;
        ser_struct.serialize_field("frontrun_swaps.pool", &frontrun_swaps.pool)?;
        ser_struct.serialize_field("frontrun_swaps.token_in", &frontrun_swaps.token_in)?;
        ser_struct.serialize_field("frontrun_swaps.token_out", &frontrun_swaps.token_out)?;
        ser_struct.serialize_field("frontrun_swaps.amount_in", &frontrun_swaps.amount_in)?;
        ser_struct.serialize_field("frontrun_swaps.amount_out", &frontrun_swaps.amount_out)?;

        let frontrun_gas_details = (
            self.frontrun_gas_details.coinbase_transfer,
            self.frontrun_gas_details.priority_fee,
            self.frontrun_gas_details.gas_used,
            self.frontrun_gas_details.effective_gas_price,
        );
        //serializer.seri
        ser_struct.serialize_field("frontrun_gas_details", &(frontrun_gas_details))?;

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
        ser_struct.serialize_field(
            "backrun_tx_hash",
            &FixedString::from(format!("{:?}", self.backrun_tx_hash)),
        )?;

        let backrun_swaps: ClickhouseVecNormalizedSwap = self.backrun_swaps.clone().into();

        ser_struct.serialize_field("backrun_swaps.trace_idx", &backrun_swaps.trace_index)?;
        ser_struct.serialize_field("backrun_swaps.from", &backrun_swaps.from)?;
        ser_struct.serialize_field("backrun_swaps.recipient", &backrun_swaps.recipient)?;
        ser_struct.serialize_field("backrun_swaps.pool", &backrun_swaps.pool)?;
        ser_struct.serialize_field("backrun_swaps.token_in", &backrun_swaps.token_in)?;
        ser_struct.serialize_field("backrun_swaps.token_out", &backrun_swaps.token_out)?;
        ser_struct.serialize_field("backrun_swaps.amount_in", &backrun_swaps.amount_in)?;
        ser_struct.serialize_field("backrun_swaps.amount_out", &backrun_swaps.amount_out)?;

        let backrun_gas_details = (
            self.backrun_gas_details.coinbase_transfer,
            self.backrun_gas_details.priority_fee,
            self.backrun_gas_details.gas_used,
            self.backrun_gas_details.effective_gas_price,
        );
        ser_struct.serialize_field("backrun_gas_details", &(backrun_gas_details))?;

        ser_struct.end()
    }
}

impl DbRow for Sandwich {
    const COLUMN_NAMES: &'static [&'static str] = &[
        "frontrun_tx_hash",
        "frontrun_swaps.trace_idx",
        "frontrun_swaps.from",
        "frontrun_swaps.recipient",
        "frontrun_swaps.pool",
        "frontrun_swaps.token_in",
        "frontrun_swaps.token_out",
        "frontrun_swaps.amount_in",
        "frontrun_swaps.amount_out",
        "frontrun_gas_details",
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
        "backrun_swaps.trace_idx",
        "backrun_swaps.from",
        "backrun_swaps.recipient",
        "backrun_swaps.pool",
        "backrun_swaps.token_in",
        "backrun_swaps.token_out",
        "backrun_swaps.amount_in",
        "backrun_swaps.amount_out",
        "backrun_gas_details",
    ];
}
