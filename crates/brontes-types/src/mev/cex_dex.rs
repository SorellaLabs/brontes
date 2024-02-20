use std::{fmt, fmt::Debug};

use ::clickhouse::DbRow;
use ::serde::ser::{SerializeStruct, Serializer};
#[allow(unused)]
use clickhouse::fixed_string::FixedString;
use malachite::Rational;
use redefined::Redefined;
use reth_primitives::B256;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use super::{Mev, MevType};
use crate::{
    db::{
        cex::CexExchange,
        redefined_types::{malachite::RationalRedefined, primitives::*},
    },
    normalized_actions::*,
    Protocol, ToFloatNearest,
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
pub struct CexDex {
    pub tx_hash:          B256,
    pub swaps:            Vec<NormalizedSwap>,
    pub stat_arb_details: Vec<StatArbDetails>,
    pub pnl:              StatArbPnl,
    #[redefined(same_fields)]
    pub gas_details:      GasDetails,
}

impl Mev for CexDex {
    fn mev_type(&self) -> MevType {
        MevType::CexDex
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
}

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

        let stat_arb_details: ClickhouseVecStatArbDetails = self.stat_arb_details.clone().into();

        ser_struct
            .serialize_field("stat_arb_details.cex_exchange", &stat_arb_details.cex_exchange)?;
        ser_struct.serialize_field("stat_arb_details.cex_price", &stat_arb_details.cex_price)?;
        ser_struct
            .serialize_field("stat_arb_details.dex_exchange", &stat_arb_details.dex_exchange)?;
        ser_struct.serialize_field("stat_arb_details.dex_price", &stat_arb_details.dex_price)?;
        ser_struct.serialize_field(
            "stat_arb_details.pre_gas_maker_profit",
            &stat_arb_details.pnl_maker_profit,
        )?;

        ser_struct.serialize_field(
            "stat_arb_details.pre_gas_taker_profit",
            &stat_arb_details.pnl_taker_profit,
        )?;

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
        "stat_arb_details.cex_exchange",
        "stat_arb_details.cex_price",
        "stat_arb_details.dex_exchange",
        "stat_arb_details.dex_price",
        "stat_arb_details.pre_gas_maker_profit",
        "stat_arb_details.pre_gas_taker_profit",
        "pnl.taker_profit",
        "pnl.maker_profit",
        "gas_details",
    ];
}

#[serde_as]
#[derive(Debug, Deserialize, PartialEq, Clone, Default, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct StatArbDetails {
    #[redefined(same_fields)]
    pub cex_exchange: CexExchange,
    pub cex_price:    Rational,
    #[redefined(same_fields)]
    pub dex_exchange: Protocol,
    pub dex_price:    Rational,
    // Arbitrage profit considering both CEX and DEX swap fees, before applying gas fees
    pub pnl_pre_gas:  StatArbPnl,
}

impl fmt::Display for StatArbDetails {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Arb Leg Details:")?;
        writeln!(f, "   - Price on {}: {}", self.cex_exchange, self.cex_price.clone().to_float())?;
        writeln!(f, "   - Price on {}: {}", self.dex_exchange, self.dex_price.clone().to_float())?;
        write!(f, "   - Pnl pre-gas: {}", self.pnl_pre_gas)
    }
}

#[serde_as]
#[derive(Debug, Deserialize, PartialEq, Clone, Default, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct StatArbPnl {
    pub maker_profit: Rational,
    pub taker_profit: Rational,
}

impl fmt::Display for StatArbPnl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "\n - Maker: {}\n - Taker: {}",
            self.maker_profit.clone().to_float(),
            self.taker_profit.clone().to_float()
        )?;

        Ok(())
    }
}
