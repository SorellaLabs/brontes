use ::serde::ser::{Serialize, SerializeStruct, Serializer};
use itertools::Itertools;
use sorella_db_databases::clickhouse::{fixed_string::FixedString, DbRow};

use crate::{classified_mev::CexDex, serde_utils::normalized_actions::ClickhouseVecNormalizedSwap};

impl Serialize for CexDex {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ser_struct = serializer.serialize_struct("CexDex", 34)?;

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

        ser_struct.serialize_field("prices.kind", &(self.prices_kind))?;
        ser_struct.serialize_field(
            "prices.address",
            &(self
                .prices_address
                .iter()
                .map(|addr| FixedString::from(format!("{:?}", addr)))
                .collect_vec()),
        )?;
        ser_struct.serialize_field("prices.price", &(self.prices_price))?;

        ser_struct.end()
    }
}

impl DbRow for CexDex {
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
        "prices.kind",
        "prices.address",
        "prices.price",
    ];
}
