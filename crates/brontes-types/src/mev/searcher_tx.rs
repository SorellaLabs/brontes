use std::fmt::Debug;

use ::serde::ser::Serializer;
use ahash::{HashSet, HashSetExt};
use clickhouse::DbRow;
use redefined::Redefined;
use alloy_primitives::B256;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{ser::SerializeStruct, Deserialize, Serialize};
use serde_with::serde_as;

use crate::{
    db::redefined_types::primitives::*,
    mev::{Mev, MevType},
    normalized_actions::*,
    Protocol,
};
#[allow(unused_imports)]
use crate::{display::utils::display_sandwich, normalized_actions::NormalizedTransfer, GasDetails};

#[serde_as]
#[derive(Debug, Deserialize, PartialEq, Clone, Default, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct SearcherTx {
    pub tx_hash:      B256,
    pub block_number: u64,
    pub transfers:    Vec<NormalizedTransfer>,
    #[redefined(same_fields)]
    pub gas_details:  GasDetails,
}

impl Mev for SearcherTx {
    fn mev_type(&self) -> MevType {
        MevType::SearcherTx
    }

    fn mev_transaction_hashes(&self) -> Vec<B256> {
        vec![self.tx_hash]
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
        HashSet::new()
    }
}

impl Serialize for SearcherTx {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ser_struct = serializer.serialize_struct("SearcherTx", 9)?;

        ser_struct.serialize_field("tx_hash", &format!("{:?}", self.tx_hash))?;
        ser_struct.serialize_field("block_number", &self.block_number)?;

        let victim_transfer: ClickhouseVecNormalizedTransfer = self
            .transfers
            .clone()
            .try_into()
            .map_err(serde::ser::Error::custom)?;
        ser_struct.serialize_field("transfers.trace_idx", &victim_transfer.trace_index)?;
        ser_struct.serialize_field("transfers.from", &victim_transfer.from)?;
        ser_struct.serialize_field("transfers.to", &victim_transfer.to)?;
        ser_struct.serialize_field("transfers.token", &victim_transfer.token)?;
        ser_struct.serialize_field("transfers.amount", &victim_transfer.amount)?;
        ser_struct.serialize_field("transfers.fee", &victim_transfer.fee)?;

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

impl DbRow for SearcherTx {
    const COLUMN_NAMES: &'static [&'static str] = &[
        "tx_hash",
        "block_number",
        "transfers.trace_idx",
        "transfers.from",
        "transfers.to",
        "transfers.token",
        "transfers.amount",
        "transfers.fee",
        "gas_details",
    ];
}
