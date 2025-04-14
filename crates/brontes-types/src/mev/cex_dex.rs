use std::{fmt, fmt::Debug};

use ::clickhouse::DbRow;
use ::serde::{
    ser::{SerializeStruct, Serializer},
    Deserialize, Serialize,
};
use ahash::HashSet;
use alloy_primitives::B256;
use colored::Colorize;
use malachite::Rational;
use redefined::{self_convert_redefined, Redefined};
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde_with::serde_as;
use strum::Display;

use super::{Mev, MevType};
use crate::{
    db::{
        cex::CexExchange,
        redefined_types::{malachite::RationalRedefined, primitives::*},
    },
    normalized_actions::*,
    pair::{Pair, PairRedefined},
    Protocol, ToFloatNearest,
};
#[allow(unused_imports)]
use crate::{
    display::utils::display_sandwich,
    normalized_actions::{NormalizedBurn, NormalizedLiquidation, NormalizedMint, NormalizedSwap},
    rational_to_u256_fraction, GasDetails,
};

#[derive(Debug, Deserialize, PartialEq, Clone, Default, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct OptimisticTrade {
    #[redefined(same_fields)]
    pub exchange: CexExchange,
    pub pair: Pair,
    pub timestamp: u64,
    pub price: Rational,
    pub volume: Rational,
}

impl Serialize for OptimisticTrade {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Serialize::serialize(
            &(
                self.exchange.to_string(),
                (format!("{:?}", self.pair.0), format!("{:?}", self.pair.1)),
                self.timestamp,
                self.price.clone().to_float(),
                self.volume.clone().to_float(),
            ),
            serializer,
        )
    }
}

#[serde_as]
#[derive(Debug, Deserialize, PartialEq, Clone, Default, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct CexDex {
    pub tx_hash: B256,
    pub block_timestamp: u64,
    pub block_number: u64,
    #[redefined(same_fields)]
    pub header_pnl_methodology: CexMethodology,
    pub swaps: Vec<NormalizedSwap>,
    pub global_vmap_details: Vec<ArbDetails>,
    pub global_vmap_pnl_maker: Rational,
    pub global_vmap_pnl_taker: Rational,
    pub optimal_route_details: Vec<ArbDetails>,
    pub optimal_route_pnl_maker: Rational,
    pub optimal_route_pnl_taker: Rational,
    pub optimistic_route_details: Vec<ArbDetails>,
    pub optimistic_trade_details: Vec<Vec<OptimisticTrade>>,
    pub optimistic_route_pnl_maker: Rational,
    pub optimistic_route_pnl_taker: Rational,
    pub per_exchange_details: Vec<Vec<ArbDetails>>,
    #[redefined(field((CexExchange, same)))]
    pub per_exchange_pnl: Vec<(CexExchange, (Rational, Rational))>,
    #[redefined(same_fields)]
    pub gas_details: GasDetails,
}

impl Mev for CexDex {
    fn mev_type(&self) -> MevType {
        MevType::CexDexTrades
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

#[derive(
    Copy,
    Display,
    Default,
    Debug,
    Clone,
    Eq,
    PartialEq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
)]
pub enum CexMethodology {
    GlobalWWAP,
    OptimalRouteVWAP,
    Optimistic,
    #[default]
    None,
}

self_convert_redefined!(CexMethodology);
impl Serialize for CexDex {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ser_struct = serializer.serialize_struct("CexDex", 68)?;

        ser_struct.serialize_field("tx_hash", &format!("{:?}", self.tx_hash))?;
        ser_struct.serialize_field("block_timestamp", &self.block_timestamp)?;
        ser_struct.serialize_field("block_number", &self.block_number)?;
        ser_struct
            .serialize_field("header_pnl_methodology", &self.header_pnl_methodology.to_string())?;

        let swaps: ClickhouseVecNormalizedSwap = self
            .swaps
            .clone()
            .try_into()
            .map_err(::serde::ser::Error::custom)?;

        ser_struct.serialize_field("swaps.trace_idx", &swaps.trace_index)?;
        ser_struct.serialize_field("swaps.from", &swaps.from)?;
        ser_struct.serialize_field("swaps.recipient", &swaps.recipient)?;
        ser_struct.serialize_field("swaps.pool", &swaps.pool)?;
        ser_struct.serialize_field("swaps.token_in", &swaps.token_in)?;
        ser_struct.serialize_field("swaps.token_out", &swaps.token_out)?;
        ser_struct.serialize_field("swaps.amount_in", &swaps.amount_in)?;
        ser_struct.serialize_field("swaps.amount_out", &swaps.amount_out)?;

        let transposed: ArbDetailsTransposed = self.global_vmap_details.clone().into();
        ser_struct.serialize_field(
            "global_vmap_details.pairs",
            &transposed
                .pairs
                .iter()
                .map(|p| {
                    p.iter()
                        .map(|p| (format!("{:?}", p.0), format!("{:?}", p.1)))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<Vec<_>>>(),
        )?;
        ser_struct.serialize_field(
            "global_vmap_details.trade_start_time",
            &transposed.trade_start_time,
        )?;
        ser_struct
            .serialize_field("global_vmap_details.trade_end_time", &transposed.trade_end_time)?;
        ser_struct.serialize_field(
            "global_vmap_details.cex_exchange",
            &transposed
                .cex_exchange
                .iter()
                .map(|ex| (*ex).to_string())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "global_vmap_details.price_maker",
            &transposed
                .price_maker
                .iter()
                .filter_map(|r| rational_to_u256_fraction(r).ok())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "global_vmap_details.price_taker",
            &transposed
                .price_taker
                .iter()
                .filter_map(|r| rational_to_u256_fraction(r).ok())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "global_vmap_details.dex_exchange",
            &transposed
                .dex_exchange
                .iter()
                .map(|e| (*e).to_string())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "global_vmap_details.dex_price",
            &transposed
                .dex_price
                .iter()
                .filter_map(|r| rational_to_u256_fraction(r).ok())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "global_vmap_details.dex_amount",
            &transposed
                .dex_amount
                .iter()
                .filter_map(|r| rational_to_u256_fraction(r).ok())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "global_vmap_details.pnl_maker",
            &transposed
                .pnl_maker
                .iter()
                .filter_map(|r| rational_to_u256_fraction(r).ok())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "global_vmap_details.pnl_taker",
            &transposed
                .pnl_taker
                .iter()
                .filter_map(|r| rational_to_u256_fraction(r).ok())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "global_vmap_pnl_maker",
            &rational_to_u256_fraction(&self.global_vmap_pnl_maker).unwrap_or_default(),
        )?;
        ser_struct.serialize_field(
            "global_vmap_pnl_taker",
            &rational_to_u256_fraction(&self.global_vmap_pnl_taker).unwrap_or_default(),
        )?;

        let transposed: ArbDetailsTransposed = self.optimal_route_details.clone().into();
        ser_struct.serialize_field(
            "optimal_route_details.pairs",
            &transposed
                .pairs
                .iter()
                .map(|p| {
                    p.iter()
                        .map(|p| (format!("{:?}", p.0), format!("{:?}", p.1)))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<Vec<_>>>(),
        )?;
        ser_struct.serialize_field(
            "optimal_route_details.trade_start_time",
            &transposed.trade_start_time,
        )?;
        ser_struct
            .serialize_field("optimal_route_details.trade_end_time", &transposed.trade_end_time)?;
        ser_struct.serialize_field(
            "optimal_route_details.cex_exchange",
            &transposed
                .cex_exchange
                .iter()
                .map(|ex| (*ex).to_string())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "optimal_route_details.price_maker",
            &transposed
                .price_maker
                .iter()
                .filter_map(|r| rational_to_u256_fraction(r).ok())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "optimal_route_details.price_taker",
            &transposed
                .price_taker
                .iter()
                .filter_map(|r| rational_to_u256_fraction(r).ok())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "optimal_route_details.dex_exchange",
            &transposed
                .dex_exchange
                .iter()
                .map(|e| (*e).to_string())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "optimal_route_details.dex_price",
            &transposed
                .dex_price
                .iter()
                .filter_map(|r| rational_to_u256_fraction(r).ok())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "optimal_route_details.dex_amount",
            &transposed
                .dex_amount
                .iter()
                .filter_map(|r| rational_to_u256_fraction(r).ok())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "optimal_route_details.pnl_maker",
            &transposed
                .pnl_maker
                .iter()
                .filter_map(|r| rational_to_u256_fraction(r).ok())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "optimal_route_details.pnl_taker",
            &transposed
                .pnl_taker
                .iter()
                .filter_map(|r| rational_to_u256_fraction(r).ok())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "optimal_route_pnl_maker",
            &rational_to_u256_fraction(&self.optimal_route_pnl_maker).unwrap_or_default(),
        )?;
        ser_struct.serialize_field(
            "optimal_route_pnl_taker",
            &rational_to_u256_fraction(&self.optimal_route_pnl_taker).unwrap_or_default(),
        )?;

        let transposed: ArbDetailsTransposed = self.optimistic_route_details.clone().into();
        ser_struct.serialize_field(
            "optimistic_route_details.pairs",
            &transposed
                .pairs
                .iter()
                .map(|p| {
                    p.iter()
                        .map(|p| (format!("{:?}", p.0), format!("{:?}", p.1)))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<Vec<_>>>(),
        )?;
        ser_struct.serialize_field(
            "optimistic_route_details.trade_start_time",
            &transposed.trade_start_time,
        )?;
        ser_struct.serialize_field(
            "optimistic_route_details.trade_end_time",
            &transposed.trade_end_time,
        )?;
        ser_struct.serialize_field(
            "optimistic_route_details.cex_exchange",
            &transposed
                .cex_exchange
                .iter()
                .map(|ex| (*ex).to_string())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "optimistic_route_details.price_maker",
            &transposed
                .price_maker
                .iter()
                .filter_map(|r| rational_to_u256_fraction(r).ok())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "optimistic_route_details.price_taker",
            &transposed
                .price_taker
                .iter()
                .filter_map(|r| rational_to_u256_fraction(r).ok())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "optimistic_route_details.dex_exchange",
            &transposed
                .dex_exchange
                .iter()
                .map(|e| (*e).to_string())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "optimistic_route_details.dex_price",
            &transposed
                .dex_price
                .iter()
                .filter_map(|r| rational_to_u256_fraction(r).ok())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "optimistic_route_details.dex_amount",
            &transposed
                .dex_amount
                .iter()
                .filter_map(|r| rational_to_u256_fraction(r).ok())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "optimistic_route_details.pnl_maker",
            &transposed
                .pnl_maker
                .iter()
                .filter_map(|r| rational_to_u256_fraction(r).ok())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "optimistic_route_details.pnl_taker",
            &transposed
                .pnl_taker
                .iter()
                .filter_map(|r| rational_to_u256_fraction(r).ok())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "optimistic_trade_details",
            &self
                .optimistic_trade_details
                .clone()
                .into_iter()
                .map(|details| {
                    details
                        .into_iter()
                        .map(|d| {
                            (
                                d.exchange.to_string(),
                                (format!("{}", d.pair.0), format!("{}", d.pair.1)),
                                d.timestamp,
                                rational_to_u256_fraction(&d.price).unwrap(),
                                rational_to_u256_fraction(&d.volume).unwrap(),
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "optimistic_route_pnl_maker",
            &rational_to_u256_fraction(&self.optimistic_route_pnl_maker).unwrap_or_default(),
        )?;
        ser_struct.serialize_field(
            "optimistic_route_pnl_taker",
            &rational_to_u256_fraction(&self.optimistic_route_pnl_taker).unwrap_or_default(),
        )?;

        let mut pairs = Vec::new();
        let mut start_time = Vec::new();
        let mut end_time = Vec::new();
        let mut cex_exchange = Vec::new();
        let mut price_maker = Vec::new();
        let mut price_taker = Vec::new();
        let mut dex_exchange = Vec::new();
        let mut dex_price = Vec::new();
        let mut dex_amount = Vec::new();
        let mut pnl_maker = Vec::new();
        let mut pnl_taker = Vec::new();

        for exchange_details in &self.per_exchange_details {
            let exchange_transposed: ArbDetailsTransposed = exchange_details.clone().into();
            cex_exchange.push(
                exchange_transposed
                    .cex_exchange
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>(),
            );
            pairs.push(
                exchange_transposed
                    .pairs
                    .into_iter()
                    .map(|p| {
                        p.into_iter()
                            .map(|p| (format!("{:?}", p.0), format!("{:?}", p.1)))
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>(),
            );
            start_time.push(exchange_transposed.trade_start_time);
            end_time.push(exchange_transposed.trade_end_time);
            price_maker.push(exchange_transposed.price_maker);
            price_taker.push(exchange_transposed.price_taker);
            dex_exchange.push(
                exchange_transposed
                    .dex_exchange
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>(),
            );
            dex_price.push(exchange_transposed.dex_price);
            dex_amount.push(exchange_transposed.dex_amount);
            pnl_maker.push(exchange_transposed.pnl_maker);
            pnl_taker.push(exchange_transposed.pnl_taker);
        }

        ser_struct.serialize_field("per_exchange_details.pairs", &pairs)?;
        ser_struct.serialize_field("per_exchange_details.trade_start_time", &start_time)?;
        ser_struct.serialize_field("per_exchange_details.trade_end_time", &end_time)?;
        ser_struct.serialize_field("per_exchange_details.cex_exchange", &cex_exchange)?;
        ser_struct.serialize_field(
            "per_exchange_details.price_maker",
            &price_maker
                .iter()
                .map(|f| {
                    f.iter()
                        .filter_map(|r| rational_to_u256_fraction(r).ok())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "per_exchange_details.price_taker",
            &price_taker
                .iter()
                .map(|f| {
                    f.iter()
                        .filter_map(|r| rational_to_u256_fraction(r).ok())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field("per_exchange_details.dex_exchange", &dex_exchange)?;
        ser_struct.serialize_field(
            "per_exchange_details.dex_price",
            &dex_price
                .iter()
                .map(|f| {
                    f.iter()
                        .filter_map(|r| rational_to_u256_fraction(r).ok())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "per_exchange_details.dex_amount",
            &dex_amount
                .iter()
                .map(|f| {
                    f.iter()
                        .filter_map(|r| rational_to_u256_fraction(r).ok())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "per_exchange_details.pnl_maker",
            &pnl_maker
                .iter()
                .map(|f| {
                    f.iter()
                        .filter_map(|r| rational_to_u256_fraction(r).ok())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "per_exchange_details.pnl_taker",
            &pnl_taker
                .iter()
                .map(|f| {
                    f.iter()
                        .filter_map(|r| rational_to_u256_fraction(r).ok())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
        )?;

        // Serialize per_exchange_pnl
        let (cex_ex, pnl_maker, pnl_taker): (Vec<_>, Vec<_>, Vec<_>) = self
            .per_exchange_pnl
            .iter()
            .map(|(exchange, (maker, taker))| {
                (
                    exchange.to_string(),
                    rational_to_u256_fraction(maker).unwrap_or_default(),
                    rational_to_u256_fraction(taker).unwrap_or_default(),
                )
            })
            .fold((Vec::new(), Vec::new(), Vec::new()), |mut acc, (ex, maker, taker)| {
                acc.0.push(ex);
                acc.1.push(maker);
                acc.2.push(taker);
                acc
            });

        ser_struct.serialize_field("per_exchange_pnl.cex_exchange", &cex_ex)?;
        ser_struct.serialize_field("per_exchange_pnl.pnl_maker", &pnl_maker)?;
        ser_struct.serialize_field("per_exchange_pnl.pnl_taker", &pnl_taker)?;

        let gas_details = (
            self.gas_details.coinbase_transfer,
            self.gas_details.priority_fee,
            self.gas_details.gas_used,
            self.gas_details.effective_gas_price,
        );

        ser_struct.serialize_field("gas_details", &gas_details)?;

        ser_struct.end()
    }
}

impl DbRow for CexDex {
    const COLUMN_NAMES: &'static [&'static str] = &[
        "tx_hash",
        "block_timestamp",
        "block_number",
        "header_pnl_methodology",
        "swaps.trace_idx",
        "swaps.from",
        "swaps.recipient",
        "swaps.pool",
        "swaps.token_in",
        "swaps.token_out",
        "swaps.amount_in",
        "swaps.amount_out",
        "global_vmap_details.pairs",
        "global_vmap_details.trade_start_time",
        "global_vmap_details.trade_end_time",
        "global_vmap_details.cex_exchange",
        "global_vmap_details.price_maker",
        "global_vmap_details.price_taker",
        "global_vmap_details.dex_exchange",
        "global_vmap_details.dex_price",
        "global_vmap_details.dex_amount",
        "global_vmap_details.pnl_maker",
        "global_vmap_details.pnl_taker",
        "global_vmap_pnl_maker",
        "global_vmap_pnl_taker",
        "optimal_route_details.pairs",
        "optimal_route_details.trade_start_time",
        "optimal_route_details.trade_end_time",
        "optimal_route_details.cex_exchange",
        "optimal_route_details.price_maker",
        "optimal_route_details.price_taker",
        "optimal_route_details.dex_exchange",
        "optimal_route_details.dex_price",
        "optimal_route_details.dex_amount",
        "optimal_route_details.pnl_maker",
        "optimal_route_details.pnl_taker",
        "optimal_route_pnl_maker",
        "optimal_route_pnl_taker",
        "optimistic_route_details.pairs",
        "optimistic_route_details.trade_start_time",
        "optimistic_route_details.trade_end_time",
        "optimistic_route_details.cex_exchange",
        "optimistic_route_details.price_maker",
        "optimistic_route_details.price_taker",
        "optimistic_route_details.dex_exchange",
        "optimistic_route_details.dex_price",
        "optimistic_route_details.dex_amount",
        "optimistic_route_details.pnl_maker",
        "optimistic_route_details.pnl_taker",
        "optimistic_trade_details",
        "optimistic_route_pnl_maker",
        "optimistic_route_pnl_taker",
        "per_exchange_details.pairs",
        "per_exchange_details.trade_start_time",
        "per_exchange_details.trade_end_time",
        "per_exchange_details.cex_exchange",
        "per_exchange_details.price_maker",
        "per_exchange_details.price_taker",
        "per_exchange_details.dex_exchange",
        "per_exchange_details.dex_price",
        "per_exchange_details.dex_amount",
        "per_exchange_details.pnl_maker",
        "per_exchange_details.pnl_taker",
        "per_exchange_pnl.cex_exchange",
        "per_exchange_pnl.pnl_maker",
        "per_exchange_pnl.pnl_taker",
        "gas_details",
    ];
}

#[serde_as]
#[derive(
    Debug, Deserialize, PartialEq, Clone, Default, Redefined, brontes_macros::Transposable,
)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct ArbDetails {
    pub pairs: Vec<Pair>,
    pub trade_start_time: u64,
    pub trade_end_time: u64,
    #[redefined(same_fields)]
    pub cex_exchange: CexExchange,
    pub price_maker: Rational,
    pub price_taker: Rational,
    #[redefined(same_fields)]
    pub dex_exchange: Protocol,
    pub dex_price: Rational,
    pub dex_amount: Rational,
    pub pnl_maker: Rational,
    pub pnl_taker: Rational,
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
            "       - CEX Prices: Maker: {:.6}, Taker: {:.6} (Spread: {:.5})",
            self.price_maker.clone().to_float().to_string(),
            self.price_taker.clone().to_float().to_string(),
            (&self.price_maker - &self.price_taker)
                .to_float()
                .to_string()
        )?;
        writeln!(f, "       - {}", "PnL Pre-Gas:".bold().underline().green())?;
        writeln!(
            f,
            "           - Maker PnL: {:.6}, Taker PnL: {:.6}",
            self.pnl_maker.clone().to_float().to_string(),
            self.pnl_taker.clone().to_float().to_string()
        )?;
        Ok(())
    }
}
