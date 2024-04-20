use std::{
    cmp::Ordering,
    fmt,
    fmt::Debug,
    ops::{Add, AddAssign},
};

use ::clickhouse::DbRow;
use ::serde::ser::Serializer;
use ahash::HashSet;
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
pub struct CexDexTrades {
    pub tx_hash:               B256,
    pub swaps:                 Vec<NormalizedSwap>,
    // Represents the arb details, using the cross exchange VMAP quote
    pub global_details:   Vec<TradeArbDetails>,
    pub global_pnl:       TradeArbPnl,
    // Arb details taking the most optimal route across all exchanges
    pub optimal_route_details: Vec<TradeArbDetails>,
    pub optimal_route_pnl:     TradeArbPnl,
    // Arb details using quotes from each exchange for each leg
    pub per_exchange_details:  Vec<Vec<TradeArbDetails>>,
    #[redefined(field((CexExchange, same)))]
    pub per_exchange_pnl:      Vec<(CexExchange, TradeArbPnl)>,
    #[redefined(same_fields)]
    pub gas_details:           GasDetails,
}

impl Mev for CexDexTrades {
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

    fn protocols(&self) -> HashSet<Protocol> {
        self.swaps.iter().map(|swap| swap.protocol).collect()
    }
}

impl Serialize for CexDexTrades {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        todo!();
        /*
        let mut ser_struct = serializer.serialize_struct("CexDex", 34)?;

        ser_struct.serialize_field("tx_hash", &format!("{:?}", self.tx_hash))?;

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

        let stat_arb_details: ClickhouseVecStatArbDetails = self
            .stat_arb_details
            .clone()
            .try_into()
            .map_err(serde::ser::Error::custom)?;

        ser_struct
            .serialize_field("stat_arb_details.cex_exchange", &stat_arb_details.cex_exchanges)?;
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

        let maker_profit: ([u8; 32], [u8; 32]) =
            rational_to_u256_fraction(&self.pnl.maker_profit).map_err(serde::ser::Error::custom)?;
        let taker_profit: ([u8; 32], [u8; 32]) =
            rational_to_u256_fraction(&self.pnl.taker_profit).map_err(serde::ser::Error::custom)?;

        ser_struct.serialize_field("pnl", &(maker_profit, taker_profit))?;

        let gas_details = (
            self.gas_details.coinbase_transfer,
            self.gas_details.priority_fee,
            self.gas_details.gas_used,
            self.gas_details.effective_gas_price,
        );

        ser_struct.serialize_field("gas_details", &(gas_details))?;

        ser_struct.end()  */
    }
}

impl DbRow for CexDexTrades {
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
        "pnl",
        "gas_details",
    ];
}

#[serde_as]
#[derive(Debug, Deserialize, PartialEq, Clone, Default, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct TradeArbDetails {
    #[redefined(same_fields)]
    pub cex_exchange: CexExchange,
    pub best_maker:   Rational,
    pub best_taker:   Rational,
    #[redefined(same_fields)]
    pub dex_exchange: Protocol,
    pub dex_price:    Rational,
    pub dex_amount:   Rational,
    // Arbitrage profit considering both CEX and DEX swap fees, before applying gas fees
    pub pnl_pre_gas:  TradeArbPnl,
}

impl fmt::Display for TradeArbDetails {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Arb Leg Details:")?;
        writeln!(f, "   - Price on CEX ({:?}): {}", self.cex_exchange, self.best_maker)?;
        writeln!(f, "   - Price on DEX {}: {}", self.dex_exchange, self.dex_price)?;
        writeln!(f, "   - Amount: {}", self.dex_amount)?;
        writeln!(
            f,
            "   - PnL pre-gas: Maker: {}, Taker: {}",
            self.pnl_pre_gas.maker, self.pnl_pre_gas.taker
        )?;
    }
}

#[serde_as]
#[derive(Debug, Deserialize, PartialEq, Clone, Default, Redefined, Eq)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct TradeArbPnl {
    pub maker: Rational,
    pub taker: Rational,
}

impl PartialOrd for TradeArbPnl {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.maker.cmp(&other.maker))
    }
}

impl Add for TradeArbPnl {
    type Output = Self;

    fn add(mut self, other: Self) -> Self::Output {
        self.maker += other.maker;
        self.taker += other.taker;
        self
    }
}

impl AddAssign for TradeArbPnl {
    fn add_assign(&mut self, other: Self) {
        self.maker += other.maker;
        self.taker += other.taker;
    }
}

impl Ord for TradeArbPnl {
    fn cmp(&self, other: &Self) -> Ordering {
        self.maker.0.cmp(&other.maker)
    }
}

impl fmt::Display for TradeArbPnl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ArbPnl:\n - Maker: {}\n - Taker: {}",
            self.maker.to_float(),
            self.taker.to_float()
        )?;

        Ok(())
    }
}
