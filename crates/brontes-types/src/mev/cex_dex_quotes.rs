use std::fmt::Debug;

use ::clickhouse::DbRow;
use ::serde::{
    ser::{SerializeStruct, Serializer},
    Deserialize, Serialize,
};
use ahash::HashSet;
use redefined::Redefined;
use alloy_primitives::B256;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde_with::serde_as;

use super::{Mev, MevType};
use crate::{
    db::{cex::CexExchange, redefined_types::primitives::*},
    normalized_actions::*,
    GasDetails, Protocol,
};

#[serde_as]
#[derive(Debug, Deserialize, PartialEq, Clone, Default, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct CexDexQuote {
    pub tx_hash:           B256,
    pub block_timestamp:   u64,
    pub block_number:      u64,
    pub swaps:             Vec<NormalizedSwap>,
    pub instant_mid_price: Vec<f64>,
    pub t2_mid_price:      Vec<f64>,
    pub t12_mid_price:     Vec<f64>,
    pub t30_mid_price:     Vec<f64>,
    pub t60_mid_price:     Vec<f64>,
    pub t300_mid_price:    Vec<f64>,
    #[redefined(same_fields)]
    pub exchange:          CexExchange,
    pub pnl:               f64,
    #[redefined(same_fields)]
    pub gas_details:       GasDetails,
}

impl Mev for CexDexQuote {
    fn mev_type(&self) -> MevType {
        MevType::CexDexQuotes
    }

    fn total_gas_paid(&self) -> u128 {
        self.gas_details.gas_paid()
    }

    fn total_priority_fee_paid(&self, base_fee: u128) -> u128 {
        self.gas_details.priority_fee_paid(base_fee)
    }

    fn mev_transaction_hashes(&self) -> Vec<B256> {
        vec![self.tx_hash]
    }

    fn bribe(&self) -> u128 {
        self.gas_details.coinbase_transfer.unwrap_or(0)
    }

    fn protocols(&self) -> HashSet<Protocol> {
        self.swaps.iter().map(|swap| swap.protocol).collect()
    }
}

impl Serialize for CexDexQuote {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ser_struct = serializer.serialize_struct("CexDexQuote", 19)?;
        ser_struct.serialize_field("tx_hash", &format!("{:?}", self.tx_hash))?;
        ser_struct.serialize_field("block_timestamp", &self.block_timestamp)?;
        ser_struct.serialize_field("block_number", &self.block_number)?;
        let swaps: ClickhouseVecNormalizedSwap = self
            .swaps
            .clone()
            .try_into()
            .map_err(serde::ser::Error::custom)?;
        ser_struct.serialize_field("swaps.trace_idx", &swaps.trace_index)?;
        ser_struct.serialize_field("swaps.from", &swaps.from)?;
        ser_struct.serialize_field("swaps.recipient", &swaps.recipient)?;
        ser_struct.serialize_field("swaps.pool", &swaps.pool)?;
        ser_struct.serialize_field("swaps.token_in", &swaps.token_in)?;
        ser_struct.serialize_field("swaps.token_out", &swaps.token_out)?;
        ser_struct.serialize_field("swaps.amount_in", &swaps.amount_in)?;
        ser_struct.serialize_field("swaps.amount_out", &swaps.amount_out)?;
        ser_struct.serialize_field("pnl", &self.pnl)?;
        ser_struct.serialize_field("instant_mid_price", &self.instant_mid_price)?;
        ser_struct.serialize_field("t2_mid_price", &self.t2_mid_price)?;
        ser_struct.serialize_field("t12_mid_price", &self.t12_mid_price)?;
        ser_struct.serialize_field("t30_mid_price", &self.t30_mid_price)?;
        ser_struct.serialize_field("t60_mid_price", &self.t60_mid_price)?;
        ser_struct.serialize_field("t300_mid_price", &self.t300_mid_price)?;
        ser_struct.serialize_field("exchange", &self.exchange.to_string())?;
        ser_struct.serialize_field(
            "gas_details",
            &(
                self.gas_details.coinbase_transfer,
                self.gas_details.priority_fee,
                self.gas_details.gas_used,
                self.gas_details.effective_gas_price,
            ),
        )?;
        ser_struct.end()
    }
}

impl DbRow for CexDexQuote {
    const COLUMN_NAMES: &'static [&'static str] = &[
        "tx_hash",
        "block_timestamp",
        "block_number",
        "swaps.trace_idx",
        "swaps.from",
        "swaps.recipient",
        "swaps.pool",
        "swaps.token_in",
        "swaps.token_out",
        "swaps.amount_in",
        "swaps.amount_out",
        "pnl",
        "instant_mid_price",
        "t2_mid_price",
        "t12_mid_price",
        "t30_mid_price",
        "t60_mid_price",
        "t300_mid_price",
        "exchange",
        "gas_details",
    ];
}
