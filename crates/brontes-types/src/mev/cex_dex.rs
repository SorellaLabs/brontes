use std::{
    cmp::Ordering,
    fmt,
    fmt::Debug,
    ops::{Add, AddAssign},
};

use ::clickhouse::DbRow;
use ::serde::ser::Serializer;
use ahash::HashSet;
use colored::Colorize;
use malachite::Rational;
use redefined::Redefined;
use reth_primitives::B256;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{ser::SerializeStruct, Deserialize, Serialize};
use serde_with::serde_as;

use super::{Mev, MevType};
use crate::{
    db::{
        cex::CexExchange,
        redefined_types::{malachite::RationalRedefined, primitives::*},
    },
    normalized_actions::*,
    rational_to_u256_fraction, Protocol, ToFloatNearest,
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
    pub tx_hash:               B256,
    pub swaps:                 Vec<NormalizedSwap>,
    // Represents the arb details, using the cross exchange VMAP quote
    pub global_vmap_details:   Vec<ArbDetails>,
    pub global_vmap_pnl:       ArbPnl,
    // Arb details taking the most optimal route across all exchanges
    pub optimal_route_details: Vec<ArbDetails>,
    pub optimal_route_pnl:     ArbPnl,
    // Arb details using quotes from each exchange for each leg
    pub per_exchange_details:  Vec<Vec<ArbDetails>>,
    #[redefined(field((CexExchange, same)))]
    pub per_exchange_pnl:      Vec<(CexExchange, ArbPnl)>,
    #[redefined(same_fields)]
    pub gas_details:           GasDetails,
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

    fn protocols(&self) -> HashSet<Protocol> {
        self.swaps.iter().map(|swap| swap.protocol).collect()
    }
}

impl Serialize for CexDex {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ser_struct = serializer.serialize_struct("CexDex", 40)?;

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

        let transposed: ArbDetailsTransposed = self.global_vmap_details.into();

        // "global_vmap_details.cex_exchange",
        // "global_vmap_details.best_bid_maker",
        // "global_vmap_details.best_ask_maker",
        // "global_vmap_details.best_bid_taker",
        // "global_vmap_details.best_ask_taker",
        // "global_vmap_details.dex_exchange",
        // "global_vmap_details.dex_price",
        // "global_vmap_details.dex_amount",
        // "global_vmap_details.pnl_pre_gas",

        // transpose arb_details
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
        "global_vmap_details.cex_exchange",
        "global_vmap_details.best_bid_maker",
        "global_vmap_details.best_ask_maker",
        "global_vmap_details.best_bid_taker",
        "global_vmap_details.best_ask_taker",
        "global_vmap_details.dex_exchange",
        "global_vmap_details.dex_price",
        "global_vmap_details.dex_amount",
        "global_vmap_details.pnl_pre_gas",
        "global_vmap_pnl",
        "optimal_route_details.cex_exchange",
        "optimal_route_details.best_bid_maker",
        "optimal_route_details.best_ask_maker",
        "optimal_route_details.best_bid_taker",
        "optimal_route_details.best_ask_taker",
        "optimal_route_details.dex_exchange",
        "optimal_route_details.dex_price",
        "optimal_route_details.dex_amount",
        "optimal_route_details.pnl_pre_gas",
        "optimal_route_pnl",
        "per_exchange_details.cex_exchange",
        "per_exchange_details.best_bid_maker",
        "per_exchange_details.best_ask_maker",
        "per_exchange_details.best_bid_taker",
        "per_exchange_details.best_ask_taker",
        "per_exchange_details.dex_exchange",
        "per_exchange_details.dex_price",
        "per_exchange_details.dex_amount",
        "per_exchange_details.pnl_pre_gas",
        "per_exchange_pnl.cex_exchange",
        "per_exchange_pnl.arb_pnl",
        "gas_details",
    ];
}

#[serde_as]
#[derive(
    Debug, Deserialize, PartialEq, Clone, Default, Redefined, brontes::macros::Transposable,
)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct ArbDetails {
    #[redefined(same_fields)]
    pub cex_exchange:   CexExchange,
    pub best_bid_maker: Rational,
    pub best_ask_maker: Rational,
    pub best_bid_taker: Rational,
    pub best_ask_taker: Rational,
    #[redefined(same_fields)]
    pub dex_exchange:   Protocol,
    pub dex_price:      Rational,
    pub dex_amount:     Rational,
    // Arbitrage profit considering both CEX and DEX swap fees, before applying gas fees
    pub pnl_pre_gas:    ArbPnl,
}

impl fmt::Display for ArbDetails {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "   - {}: {}",
            "Exchange".bold().underline().cyan(),
            self.cex_exchange.to_string().bold()
        )?;
        writeln!(f, "       - Dex Price: {:.6}", self.dex_price.clone().to_float().to_string())?;
        writeln!(
            f,
            "       - CEX Prices: Maker Bid: {:.6} (-{:.5}), Maker Ask: {:.6} (-{:.5})",
            self.best_bid_maker.clone().to_float().to_string(),
            (&self.best_bid_maker - &self.best_bid_taker)
                .to_float()
                .to_string(),
            self.best_ask_maker.clone().to_float().to_string(),
            (&self.best_ask_maker - &self.best_ask_taker)
                .to_float()
                .to_string()
        )?;
        writeln!(f, "       - {}", "PnL Pre-Gas:".bold().underline().green())?;
        writeln!(
            f,
            "           - Mid Price PnL: Maker: {:.6}, Taker: {:.6}",
            self.pnl_pre_gas
                .maker_taker_mid
                .0
                .clone()
                .to_float()
                .to_string(),
            self.pnl_pre_gas
                .maker_taker_mid
                .1
                .clone()
                .to_float()
                .to_string()
        )?;
        writeln!(
            f,
            "           - Ask PnL: Maker: {:.6}, Taker: {:.6}",
            self.pnl_pre_gas
                .maker_taker_ask
                .0
                .clone()
                .to_float()
                .to_string(),
            self.pnl_pre_gas
                .maker_taker_ask
                .1
                .clone()
                .to_float()
                .to_string()
        )?;

        Ok(())
    }
}
#[serde_as]
#[derive(Debug, Deserialize, PartialEq, Clone, Default, Redefined, Eq)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct ArbPnl {
    pub maker_taker_mid: (Rational, Rational),
    pub maker_taker_ask: (Rational, Rational),
}

impl PartialOrd for ArbPnl {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Add for ArbPnl {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        ArbPnl {
            maker_taker_mid: (
                self.maker_taker_mid.0 + other.maker_taker_mid.0,
                self.maker_taker_mid.1 + other.maker_taker_mid.1,
            ),
            maker_taker_ask: (
                self.maker_taker_ask.0 + other.maker_taker_ask.0,
                self.maker_taker_ask.1 + other.maker_taker_ask.1,
            ),
        }
    }
}

impl AddAssign for ArbPnl {
    fn add_assign(&mut self, other: Self) {
        self.maker_taker_mid.0 += other.maker_taker_mid.0;
        self.maker_taker_mid.1 += other.maker_taker_mid.1;
        self.maker_taker_ask.0 += other.maker_taker_ask.0;
        self.maker_taker_ask.1 += other.maker_taker_ask.1;
    }
}

impl Ord for ArbPnl {
    fn cmp(&self, other: &Self) -> Ordering {
        self.maker_taker_mid.0.cmp(&other.maker_taker_mid.0)
    }
}

impl fmt::Display for ArbPnl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ArbPnl:\n - Maker Mid: {}\n - Taker Mid: {}\n - Maker Ask: {}\n - Taker Ask: {}",
            self.maker_taker_mid.0.clone().to_float(),
            self.maker_taker_mid.1.clone().to_float(),
            self.maker_taker_ask.0.clone().to_float(),
            self.maker_taker_ask.1.clone().to_float()
        )?;

        Ok(())
    }
}
