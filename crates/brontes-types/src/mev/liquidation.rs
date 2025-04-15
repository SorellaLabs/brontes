use std::fmt::Debug;

use ::clickhouse::DbRow;
use ::serde::ser::{SerializeStruct, Serializer};
use ahash::HashSet;
use alloy_primitives::B256;
#[allow(unused)]
use clickhouse::fixed_string::FixedString;
use redefined::Redefined;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use super::{Mev, MevType};
use crate::{db::redefined_types::primitives::*, Protocol};
#[allow(unused_imports)]
use crate::{display::utils::display_sandwich, normalized_actions::*, GasDetails};

#[serde_as]
#[derive(Debug, Deserialize, PartialEq, Clone, Default, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct Liquidation {
    pub liquidation_tx_hash: B256,
    pub block_number:        u64,
    pub trigger:             B256,
    pub liquidation_swaps:   Vec<NormalizedSwap>,
    pub liquidations:        Vec<NormalizedLiquidation>,
    #[redefined(same_fields)]
    pub gas_details:         GasDetails,
}

impl Mev for Liquidation {
    fn mev_type(&self) -> MevType {
        MevType::Liquidation
    }

    fn mev_transaction_hashes(&self) -> Vec<B256> {
        vec![self.liquidation_tx_hash]
    }

    fn total_gas_paid(&self) -> u128 {
        self.gas_details.gas_paid()
    }

    fn total_priority_fee_paid(&self, base_fee: u128) -> u128 {
        self.gas_details.priority_fee_paid(base_fee)
    }

    fn bribe(&self) -> u128 {
        self.gas_details.coinbase_transfer.unwrap_or(0)
    }

    fn protocols(&self) -> HashSet<Protocol> {
        let mut protocols: HashSet<Protocol> = self
            .liquidation_swaps
            .iter()
            .map(|swap| swap.protocol)
            .collect();

        self.liquidations.iter().for_each(|liquidation| {
            protocols.insert(liquidation.protocol);
        });

        protocols
    }
}

impl Serialize for Liquidation {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ser_struct = serializer.serialize_struct("Liquidation", 34)?;

        // frontrun
        ser_struct
            .serialize_field("liquidation_tx_hash", &format!("{:?}", self.liquidation_tx_hash))?;
        ser_struct.serialize_field("block_number", &self.block_number)?;

        let liquidation_swaps: ClickhouseVecNormalizedSwap = self
            .liquidation_swaps
            .clone()
            .try_into()
            .map_err(serde::ser::Error::custom)?;

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
        let liquidations: ClickhouseVecNormalizedLiquidation = self
            .liquidations
            .clone()
            .try_into()
            .map_err(serde::ser::Error::custom)?;

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
        "block_number",
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
