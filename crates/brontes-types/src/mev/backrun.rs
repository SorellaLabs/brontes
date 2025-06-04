use std::{
    fmt,
    fmt::{Debug, Display},
};

use ::clickhouse::DbRow;
use ::serde::ser::{SerializeStruct, Serializer};
use ahash::HashSet;
#[allow(unused)]
use clickhouse::fixed_string::FixedString;
use redefined::{self_convert_redefined, Redefined};
use reth_primitives::B256;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use super::{Mev, MevType};
use crate::{
    db::redefined_types::primitives::B256Redefined,
    normalized_actions::{ClickhouseVecNormalizedSwap, NormalizedSwap, NormalizedSwapRedefined},
    GasDetails, Protocol,
};

#[serde_as]
#[derive(Debug, Deserialize, PartialEq, Clone, Default, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct AtomicArb {
    pub tx_hash:      B256,
    pub trigger_tx:   B256,
    pub block_number: u64,
    pub swaps:        Vec<NormalizedSwap>,
    #[redefined(same_fields)]
    pub gas_details:  GasDetails,
    #[redefined(same_fields)]
    pub arb_type:     AtomicArbType,
    pub profit_usd:   f64,
    pub protocols:    Vec<String>,
}
/// Represents the different types of atomic arb
/// A triangle arb is a simple arb that goes from token A -> B -> C -> A
/// A cross pair arb is a more complex arb that goes from token A -> B -> C -> A

#[derive(
    Debug,
    Default,
    PartialEq,
    Eq,
    Clone,
    Serialize,
    Deserialize,
    rSerialize,
    rDeserialize,
    Archive,
    Copy,
)]
pub enum AtomicArbType {
    #[default]
    Triangle,
    CrossPair(usize),
    StablecoinArb,
    LongTail,
}
impl Display for AtomicArbType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AtomicArbType::Triangle => writeln!(f, "Triangular Arbitrage"),
            AtomicArbType::CrossPair(_) => writeln!(f, "Cross Pair Arbitrage"),
            AtomicArbType::StablecoinArb => writeln!(f, "Stablecoin Arbitrage"),
            AtomicArbType::LongTail => writeln!(f, "LongTail Arbitrage"),
        }
    }
}

//TODO: Ludwig, add flashloan arb support

self_convert_redefined!(AtomicArbType);

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

    fn protocols(&self) -> HashSet<Protocol> {
        self.swaps.iter().map(|swap| swap.protocol).collect()
    }
}

impl Serialize for AtomicArb {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ser_struct = serializer.serialize_struct("AtomicArb", 37)?;
        ser_struct.serialize_field("tx_hash", &format!("{:?}", self.tx_hash))?;
        ser_struct.serialize_field("block_number", &self.block_number)?;
        ser_struct.serialize_field("trigger_tx", &format!("{:?}", self.trigger_tx))?;
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
        let gas_details = (
            self.gas_details.coinbase_transfer,
            self.gas_details.priority_fee,
            self.gas_details.gas_used,
            self.gas_details.effective_gas_price,
        );
        ser_struct.serialize_field("gas_details", &gas_details)?;
        ser_struct.serialize_field("arb_type", &self.arb_type.to_string())?;
        ser_struct.serialize_field("profit_usd", &self.profit_usd)?;
        ser_struct.serialize_field("protocols", &self.protocols)?;
        ser_struct.end()
    }
}

impl DbRow for AtomicArb {
    const COLUMN_NAMES: &'static [&'static str] = &[
        "tx_hash",
        "block_number",
        "trigger_tx",
        "swaps.trace_idx",
        "swaps.from",
        "swaps.recipient",
        "swaps.pool",
        "swaps.token_in",
        "swaps.token_out",
        "swaps.amount_in",
        "swaps.amount_out",
        "gas_details",
        "arb_type",
    ];
}
