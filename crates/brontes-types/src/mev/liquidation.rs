use std::fmt::{Debug};

use ::serde::ser::{Serialize, SerializeStruct, Serializer};





use reth_primitives::B256;
use serde::Deserialize;

use serde_with::serde_as;
use sorella_db_databases::{
    clickhouse::{fixed_string::FixedString, DbRow},
};


use super::{Mev, MevType};
use crate::normalized_actions::{ClickhouseVecNormalizedLiquidation, ClickhouseVecNormalizedSwap};
#[allow(unused_imports)]
use crate::{
    display::utils::{display_sandwich, print_mev_type_header},
    normalized_actions::{NormalizedBurn, NormalizedLiquidation, NormalizedMint, NormalizedSwap},
    serde_primitives::vec_fixed_string,
    GasDetails,
};

#[serde_as]
#[derive(Debug, Deserialize, Clone, Default)]
pub struct Liquidation {
    pub liquidation_tx_hash: B256,
    pub trigger:             B256,
    pub liquidation_swaps:   Vec<NormalizedSwap>,
    pub liquidations:        Vec<NormalizedLiquidation>,
    pub gas_details:         GasDetails,
}

impl Mev for Liquidation {
    fn mev_type(&self) -> MevType {
        MevType::Liquidation
    }

    fn mev_transaction_hashes(&self) -> Vec<B256> {
        vec![self.liquidation_tx_hash]
    }

    fn priority_fee_paid(&self) -> u128 {
        self.gas_details.gas_paid()
    }

    fn bribe(&self) -> u128 {
        self.gas_details.coinbase_transfer.unwrap_or(0)
    }
}

impl Serialize for Liquidation {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ser_struct = serializer.serialize_struct("Liquidation", 34)?;

        // frontrun
        ser_struct.serialize_field(
            "liquidation_tx_hash",
            &FixedString::from(format!("{:?}", self.liquidation_tx_hash)),
        )?;

        let liquidation_swaps: ClickhouseVecNormalizedSwap = self.liquidation_swaps.clone().into();

        ser_struct
            .serialize_field("liquidation_swaps.trace_idx", &liquidation_swaps.trace_index)?;
        ser_struct.serialize_field("liquidation_swaps.from", &liquidation_swaps.from)?;
        ser_struct.serialize_field("liquidation_swaps.recipient", &liquidation_swaps.recipient)?;
        ser_struct.serialize_field("liquidation_swaps.pool", &liquidation_swaps.pool)?;
        ser_struct.serialize_field("liquidation_swaps.token_in", &liquidation_swaps.token_in)?;
        ser_struct.serialize_field("liquidation_swaps.token_out", &liquidation_swaps.token_out)?;
        ser_struct.serialize_field("liquidation_swaps.amount_in", &liquidation_swaps.amount_in)?;
        ser_struct
            .serialize_field("liquidation_swaps.amount_out", &liquidation_swaps.amount_out)?;

        // victims
        let liquidations: ClickhouseVecNormalizedLiquidation = self.liquidations.clone().into();

        ser_struct.serialize_field("liquidations.trace_idx", &liquidations.trace_index)?;
        ser_struct.serialize_field("liquidations.pool", &liquidations.pool)?;
        ser_struct.serialize_field("liquidations.liquidator", &liquidations.liquidator)?;
        ser_struct.serialize_field("liquidations.debtor", &liquidations.debtor)?;
        ser_struct
            .serialize_field("liquidations.collateral_asset", &liquidations.collateral_asset)?;
        ser_struct.serialize_field("liquidations.debt_asset", &liquidations.debt_asset)?;
        ser_struct.serialize_field("liquidations.covered_debt", &liquidations.covered_debt)?;
        ser_struct.serialize_field(
            "liquidations.liquidated_collateral",
            &liquidations.liquidated_collateral,
        )?;

        let gas_details = (
            self.gas_details.coinbase_transfer,
            self.gas_details.priority_fee,
            self.gas_details.gas_used,
            self.gas_details.effective_gas_price,
        );
        //serializer.seri
        ser_struct.serialize_field("gas_details", &(gas_details))?;

        ser_struct.end()
    }
}

impl DbRow for Liquidation {
    const COLUMN_NAMES: &'static [&'static str] = &[
        "liquidation_tx_hash",
        "liquidation_swaps.trace_idx",
        "liquidation_swaps.from",
        "liquidation_swaps.recipient",
        "liquidation_swaps.pool",
        "liquidation_swaps.token_in",
        "liquidation_swaps.token_out",
        "liquidation_swaps.amount_in",
        "liquidation_swaps.amount_out",
        "liquidations.trace_idx",
        "liquidations.pool",
        "liquidations.liquidator",
        "liquidations.debtor",
        "liquidations.collateral_asset",
        "liquidations.debt_asset",
        "liquidations.covered_debt",
        "liquidations.liquidated_collateral",
        "gas_details",
    ];
}
