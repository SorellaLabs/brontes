use std::{
    fmt,
    fmt::Debug,
    ops::{Add, AddAssign},
};

use ::clickhouse::DbRow;
use ::serde::{
    ser::{SerializeStruct, Serializer},
    Deserialize, Serialize,
};
use ahash::HashSet;
use colored::Colorize;
use malachite::Rational;
use redefined::{self_convert_redefined, Redefined};
use reth_primitives::B256;
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
    GasDetails,
};

#[derive(Debug, Deserialize, PartialEq, Clone, Default, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct OptimisticTrade {
    #[redefined(same_fields)]
    pub exchange:  CexExchange,
    pub pair:      Pair,
    pub timestamp: u64,
    pub price:     Rational,
    pub volume:    Rational,
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
    pub global_vmap_pnl_maker: f64,
    pub global_vmap_pnl_taker: f64,
    pub optimal_route_details: Vec<ArbDetails>,
    pub optimal_route_pnl_maker: f64,
    pub optimal_route_pnl_taker: f64,
    pub optimistic_route_details: Vec<ArbDetails>,
    pub optimistic_trade_details: Vec<Vec<OptimisticTrade>>,
    pub optimistic_route_pnl_maker: f64,
    pub optimistic_route_pnl_taker: f64,
    pub per_exchange_details: Vec<Vec<ArbDetails>>,
    #[redefined(field((CexExchange, same)))]
    pub per_exchange_pnl: Vec<(CexExchange, (f64, f64))>,
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
        ser_struct.serialize_field("block_number", &self.block_number)?;

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

        let global_vmap_details_transposed: ArbDetailsTransposed =
            self.global_vmap_details.clone().into();
        ser_struct.serialize_field(
            "global_vmap_details.pairs",
            &global_vmap_details_transposed
                .pairs
                .iter()
                .map(|p| {
                    p.iter()
                        .map(|pair| (format!("{:?}", pair.0), format!("{:?}", pair.1)))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "global_vmap_details.trade_start_time",
            &global_vmap_details_transposed.trade_start_time,
        )?;
        ser_struct.serialize_field(
            "global_vmap_details.trade_end_time",
            &global_vmap_details_transposed.trade_end_time,
        )?;
        ser_struct.serialize_field(
            "global_vmap_details.cex_exchange",
            &global_vmap_details_transposed
                .cex_exchange
                .iter()
                .map(|ex| ex.to_string())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "global_vmap_details.price_maker",
            &global_vmap_details_transposed.price_maker,
        )?;
        ser_struct.serialize_field(
            "global_vmap_details.price_taker",
            &global_vmap_details_transposed.price_taker,
        )?;
        ser_struct.serialize_field(
            "global_vmap_details.dex_exchange",
            &global_vmap_details_transposed
                .dex_exchange
                .iter()
                .map(|ex| ex.to_string())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "global_vmap_details.dex_price",
            &global_vmap_details_transposed.dex_price,
        )?;
        ser_struct.serialize_field(
            "global_vmap_details.dex_amount",
            &global_vmap_details_transposed.dex_amount,
        )?;
        ser_struct.serialize_field(
            "global_vmap_details.pnl_maker",
            &global_vmap_details_transposed.pnl_maker,
        )?;
        ser_struct.serialize_field(
            "global_vmap_details.pnl_taker",
            &global_vmap_details_transposed.pnl_taker,
        )?;

        ser_struct.serialize_field("global_vmap_pnl_maker", &self.global_vmap_pnl_maker)?;
        ser_struct.serialize_field("global_vmap_pnl_taker", &self.global_vmap_pnl_taker)?;

        // Serialize optimal_route_details
        let optimal_route_details_transposed: ArbDetailsTransposed =
            self.optimal_route_details.clone().into();
        ser_struct.serialize_field(
            "optimal_route_details.pairs",
            &optimal_route_details_transposed
                .pairs
                .iter()
                .map(|p| {
                    p.iter()
                        .map(|pair| (format!("{:?}", pair.0), format!("{:?}", pair.1)))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "optimal_route_details.trade_start_time",
            &optimal_route_details_transposed.trade_start_time,
        )?;
        ser_struct.serialize_field(
            "optimal_route_details.trade_end_time",
            &optimal_route_details_transposed.trade_end_time,
        )?;
        ser_struct.serialize_field(
            "optimal_route_details.cex_exchange",
            &optimal_route_details_transposed
                .cex_exchange
                .iter()
                .map(|ex| ex.to_string())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "optimal_route_details.price_maker",
            &optimal_route_details_transposed.price_maker,
        )?;
        ser_struct.serialize_field(
            "optimal_route_details.price_taker",
            &optimal_route_details_transposed.price_taker,
        )?;
        ser_struct.serialize_field(
            "optimal_route_details.dex_exchange",
            &optimal_route_details_transposed
                .dex_exchange
                .iter()
                .map(|ex| ex.to_string())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "optimal_route_details.dex_price",
            &optimal_route_details_transposed.dex_price,
        )?;
        ser_struct.serialize_field(
            "optimal_route_details.dex_amount",
            &optimal_route_details_transposed.dex_amount,
        )?;
        ser_struct.serialize_field(
            "optimal_route_details.pnl_maker",
            &optimal_route_details_transposed.pnl_maker,
        )?;
        ser_struct.serialize_field(
            "optimal_route_details.pnl_taker",
            &optimal_route_details_transposed.pnl_taker,
        )?;

        ser_struct.serialize_field("optimal_route_pnl_maker", &self.optimal_route_pnl_maker)?;
        ser_struct.serialize_field("optimal_route_pnl_taker", &self.optimal_route_pnl_taker)?;

        // Serialize optimistic_route_details
        let optimistic_route_details_transposed: ArbDetailsTransposed =
            self.optimistic_route_details.clone().into();
        ser_struct.serialize_field(
            "optimistic_route_details.pairs",
            &optimistic_route_details_transposed
                .pairs
                .iter()
                .map(|p| {
                    p.iter()
                        .map(|pair| (format!("{:?}", pair.0), format!("{:?}", pair.1)))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "optimistic_route_details.trade_start_time",
            &optimistic_route_details_transposed.trade_start_time,
        )?;
        ser_struct.serialize_field(
            "optimistic_route_details.trade_end_time",
            &optimistic_route_details_transposed.trade_end_time,
        )?;
        ser_struct.serialize_field(
            "optimistic_route_details.cex_exchange",
            &optimistic_route_details_transposed
                .cex_exchange
                .iter()
                .map(|ex| ex.to_string())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "optimistic_route_details.price_maker",
            &optimistic_route_details_transposed.price_maker,
        )?;
        ser_struct.serialize_field(
            "optimistic_route_details.price_taker",
            &optimistic_route_details_transposed.price_taker,
        )?;
        ser_struct.serialize_field(
            "optimistic_route_details.dex_exchange",
            &optimistic_route_details_transposed
                .dex_exchange
                .iter()
                .map(|ex| ex.to_string())
                .collect::<Vec<_>>(),
        )?;
        ser_struct.serialize_field(
            "optimistic_route_details.dex_price",
            &optimistic_route_details_transposed.dex_price,
        )?;
        ser_struct.serialize_field(
            "optimistic_route_details.dex_amount",
            &optimistic_route_details_transposed.dex_amount,
        )?;
        ser_struct.serialize_field(
            "optimistic_route_details.pnl_maker",
            &optimistic_route_details_transposed.pnl_maker,
        )?;
        ser_struct.serialize_field(
            "optimistic_route_details.pnl_taker",
            &optimistic_route_details_transposed.pnl_taker,
        )?;

        // Serialize optimistic_trade_details
        ser_struct.serialize_field("optimistic_trade_details", &self.optimistic_trade_details)?;

        ser_struct
            .serialize_field("optimistic_route_pnl_maker", &self.optimistic_route_pnl_maker)?;
        ser_struct
            .serialize_field("optimistic_route_pnl_taker", &self.optimistic_route_pnl_taker)?;

        // Serialize per_exchange_details
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

        for ex in &self.per_exchange_details {
            let transposed: ArbDetailsTransposed = ex.clone().into();
            cex_exchange.push(
                transposed
                    .cex_exchange
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>(),
            );
            pairs.push(
                transposed
                    .pairs
                    .into_iter()
                    .map(|p| {
                        p.into_iter()
                            .map(|p| (format!("{:?}", p.0), format!("{:?}", p.1)))
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>(),
            );
            start_time.push(transposed.trade_start_time);
            end_time.push(transposed.trade_end_time);
            price_maker.push(transposed.price_maker);
            price_taker.push(transposed.price_taker);
            dex_exchange.push(
                transposed
                    .dex_exchange
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>(),
            );
            dex_price.push(transposed.dex_price);
            dex_amount.push(transposed.dex_amount);
            pnl_maker.push(transposed.pnl_maker);
            pnl_taker.push(transposed.pnl_taker);
        }

        ser_struct.serialize_field("per_exchange_details.pairs", &pairs)?;
        ser_struct.serialize_field("per_exchange_details.trade_start_time", &start_time)?;
        ser_struct.serialize_field("per_exchange_details.trade_end_time", &end_time)?;
        ser_struct.serialize_field("per_exchange_details.cex_exchange", &cex_exchange)?;
        ser_struct.serialize_field("per_exchange_details.price_maker", &price_maker)?;
        ser_struct.serialize_field("per_exchange_details.price_taker", &price_taker)?;
        ser_struct.serialize_field("per_exchange_details.dex_exchange", &dex_exchange)?;
        ser_struct.serialize_field("per_exchange_details.dex_price", &dex_price)?;
        ser_struct.serialize_field("per_exchange_details.dex_amount", &dex_amount)?;
        ser_struct.serialize_field("per_exchange_details.pnl_maker", &pnl_maker)?;
        ser_struct.serialize_field("per_exchange_details.pnl_taker", &pnl_taker)?;

        // Serialize per_exchange_pnl
        let (cex_ex, pnl_maker, pnl_taker): (Vec<_>, Vec<_>, Vec<_>) = self
            .per_exchange_pnl
            .iter()
            .map(|(exchange, (maker, taker))| (exchange.to_string(), *maker, *taker))
            .fold(
                (Vec::new(), Vec::new(), Vec::new()),
                |(mut ex, mut maker, mut taker), (e, m, t)| {
                    ex.push(e);
                    maker.push(m);
                    taker.push(t);
                    (ex, maker, taker)
                },
            );

        ser_struct.serialize_field("per_exchange_pnl.cex_exchange", &cex_ex)?;
        ser_struct.serialize_field("per_exchange_pnl.pnl_maker", &pnl_maker)?;
        ser_struct.serialize_field("per_exchange_pnl.pnl_taker", &pnl_taker)?;

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
        "global_time_window_start",
        "global_time_window_end",
        "global_optimistic_start",
        "global_optimistic_end",
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
    pub pairs:            Vec<Pair>,
    pub trade_start_time: u64,
    pub trade_end_time:   u64,
    #[redefined(same_fields)]
    pub cex_exchange:     CexExchange,
    pub price_maker:      Rational,
    pub price_taker:      Rational,
    #[redefined(same_fields)]
    pub dex_exchange:     Protocol,
    pub dex_price:        Rational,
    pub dex_amount:       Rational,
    pub pnl_maker:        Rational,
    pub pnl_taker:        Rational,
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
