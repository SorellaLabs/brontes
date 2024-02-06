use std::fmt::Debug;

use ::serde::ser::{SerializeStruct, Serializer};
use redefined::Redefined;
use reth_primitives::B256;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sorella_db_databases::clickhouse::{fixed_string::FixedString, DbRow};

use super::{Mev, MevType};
use crate::{
    db::redefined_types::primitives::B256Redefined,
    normalized_actions::{ClickhouseVecNormalizedSwap, NormalizedSwapRedefined},
};
#[allow(unused_imports)]
use crate::{
    display::utils::display_sandwich,
    normalized_actions::{NormalizedBurn, NormalizedLiquidation, NormalizedMint, NormalizedSwap},
    GasDetails,
};

#[serde_as]
#[derive(Debug, Deserialize, PartialEq, Clone, Default, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct AtomicArb {
    pub tx_hash:     B256,
    pub swaps:       Vec<NormalizedSwap>,
    #[redefined(same_fields)]
    pub gas_details: GasDetails,
}

impl Mev for AtomicArb {
    fn total_gas_paid(&self) -> u128 {
        self.gas_details.gas_paid()
    }

    fn total_priority_fee_paid(&self, base_fee: u128) -> u128 {
        self.gas_details.priority_fee(base_fee) * self.gas_details.gas_used
    }

    fn bribe(&self) -> u128 {
        self.gas_details.coinbase_transfer.unwrap_or(0)
    }

    fn mev_transaction_hashes(&self) -> Vec<B256> {
        vec![self.tx_hash]
    }

    fn mev_type(&self) -> MevType {
        MevType::AtomicArb
    }
}

impl Serialize for AtomicArb {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ser_struct = serializer.serialize_struct("AtomicArb", 34)?;

        ser_struct.serialize_field("tx_hash", &FixedString::from(format!("{:?}", self.tx_hash)))?;

        let swaps: ClickhouseVecNormalizedSwap = self.swaps.clone().into();

        ser_struct.serialize_field("swaps.trace_idx", &swaps.trace_index)?;
        ser_struct.serialize_field("swaps.from", &swaps.from)?;
        ser_struct.serialize_field("swaps.recipient", &swaps.recipient)?;
        ser_struct.serialize_field("swaps.pool", &swaps.pool)?;
        ser_struct.serialize_field("swaps.token_in", &swaps.token_in)?;
        ser_struct.serialize_field("swaps.token_out", &swaps.token_out)?;
        ser_struct.serialize_field("swaps.amount_in", &swaps.amount_in)?;
        ser_struct.serialize_field("swaps.amount_out", &swaps.amount_out)?;

        let gas_details = (
            self.gas_details.coinbase_transfer,
            self.gas_details.priority_fee,
            self.gas_details.gas_used,
            self.gas_details.effective_gas_price,
        );

        ser_struct.serialize_field("gas_details", &(gas_details))?;

        ser_struct.end()
    }
}

impl DbRow for AtomicArb {
    const COLUMN_NAMES: &'static [&'static str] = &[
        "tx_hash",
        "swaps.trace_idx",
        "swaps.from",
        "swaps.recipient",
        "swaps.pool",
        "swaps.token_in",
        "swaps.token_out",
        "swaps.amount_in",
        "swaps.amount_out",
        "gas_details",
    ];
}
