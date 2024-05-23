use alloy_primitives::Address;
use clickhouse::Row;
use itertools::Itertools;
use malachite::Rational;
use serde::{Deserialize, Serialize};

use super::traits::LibmdbxReader;
use crate::{
    db::clickhouse_serde::pair::pair_ser,
    mev::{Bundle, BundleData, Mev, MevBlock, MevType},
    pair::Pair,
    Protocol,
};

#[derive(Debug, Clone, Serialize, Deserialize, Row)]
pub struct BlockAnalysis {
    pub block_number: u64,

    // all
    pub total_mev_profit:     f64,
    pub all_top_searcher:     Address,
    pub all_top_fund:         Address,
    pub all_average_profit:   f64,
    pub all_unique_searchers: u64,
    pub all_unique_funds:     u64,

    // atomic
    #[serde(serialize_with = "pair_ser::serialize")]
    pub most_arbed_pair:      Pair,
    pub most_arbed_pool:      Address,
    pub most_arbed_dex:       Protocol,
    pub arb_total_revenue:    f64,
    pub arb_total_profit:     f64,
    pub arb_top_searcher:     Address,
    pub arb_top_fund:         Address,
    pub arb_unique_searchers: u64,
    pub arb_unique_funds:     u64,

    // sandwich
    #[serde(serialize_with = "pair_ser::serialize")]
    pub most_sandwiched_pair:        Pair,
    pub most_sandwiched_pool:        Address,
    pub most_sandwiched_dex:         Protocol,
    pub sandwich_total_revenue:      f64,
    pub sandwich_total_profit:       f64,
    pub sandwich_total_swapper_loss: f64,
    pub sandwich_top_searcher:       Address,
    pub sandwich_unique_searchers:   u64,

    // jit
    #[serde(serialize_with = "pair_ser::serialize")]
    pub most_jit_pair:        Pair,
    pub most_jit_pool:        Address,
    pub most_jit_dex:         Protocol,
    pub jit_total_revenue:    f64,
    pub jit_total_profit:     f64,
    pub jit_top_searcher:     Address,
    pub jit_unique_searchers: u64,

    // jit-sandwich
    #[serde(serialize_with = "pair_ser::serialize")]
    pub most_jit_sandwiched_pair:        Pair,
    pub most_jit_sandwiched_pool:        Address,
    pub most_jit_sandwiched_dex:         Protocol,
    pub jit_sandwich_total_revenue:      f64,
    pub jit_sandwich_total_profit:       f64,
    pub jit_sandwich_total_swapper_loss: f64,
    pub jit_sandwich_top_searcher:       Address,
    pub jit_sandwich_unique_searchers:   u64,

    // cex dex
    #[serde(serialize_with = "pair_ser::serialize")]
    pub cex_dex_most_arb_pair_rev:    Pair,
    pub cex_dex_most_arb_pool_rev:    Address,
    #[serde(serialize_with = "pair_ser::serialize")]
    pub cex_dex_most_arb_pair_profit: Pair,
    pub cex_dex_most_arb_pool_profit: Address,
    pub cex_dex_total_rev:            f64,
    pub cex_dex_total_profit:         f64,
    pub cex_top_searcher:             Address,
    pub cex_top_fund:                 Address,

    // liquidation
    pub most_liquidated_token:             Address,
    pub most_liquidated_protocol:          Protocol,
    pub liquidation_total_revenue:         f64,
    pub liquidation_total_profit:          f64,
    pub liquidation_average_profit_margin: f64,
    pub liquidation_top_searcher:          Address,
    pub liquidation_unique_searchers:      u64,
    pub total_usd_liquidated:              f64,
}

impl BlockAnalysis {
    pub fn new<DB: LibmdbxReader>(block: &MevBlock, bundles: &[Bundle], db: DB) -> Self {
        Self {
            block_number:         block.block_number,
            total_mev_profit:     block.total_mev_profit_usd,
            all_unique_funds:     bundles
                .iter()
                .filter_map(|b| {
                    let eoa = db.try_fetch_searcher_eoa_info(b.header.eoa).unwrap()?;
                    if eoa.fund.is_none() {
                        let contract = db
                            .try_fetch_searcher_contract_info(b.header.mev_contract?)
                            .unwrap()?;
                        (!contract.fund.is_none()).then_some(contract.fund)
                    } else {
                        Some(eoa.fund)
                    }
                })
                .unique()
                .count() as u64,
            all_unique_searchers: bundles.iter().map(|b| b.header.eoa).unique().count() as u64,
            all_top_fund:         bundles
                .iter()
                .filter(|b| {
                    let Some(eoa) = db.try_fetch_searcher_eoa_info(b.header.eoa).unwrap() else {
                        return false
                    };
                    if eoa.fund.is_none() {
                        let Some(mev_contract) = b.header.mev_contract else { return false };
                        let Some(contract) =
                            db.try_fetch_searcher_contract_info(mev_contract).unwrap()
                        else {
                            return false
                        };
                        !contract.fund.is_none()
                    } else {
                        true
                    }
                })
                .max_by(|a, b| a.header.profit_usd.total_cmp(&b.header.profit_usd))
                .map(|h| h.header.eoa)
                .unwrap_or_default(),
            all_top_searcher:     bundles
                .iter()
                .max_by(|a, b| a.header.profit_usd.total_cmp(&b.header.profit_usd))
                .map(|r| r.header.eoa)
                .unwrap_or_default(),
            all_average_profit:   bundles.iter().map(|h| h.header.profit_usd).sum::<f64>()
                / Some(bundles.len())
                    .filter(|f| *f != 0)
                    .map(|f| f as f64)
                    .unwrap_or(1.0),

            arb_top_fund: bundles
                .iter()
                .filter(|b| {
                    if b.data.mev_type() != MevType::AtomicArb {
                        return false
                    }

                    let Some(eoa) = db.try_fetch_searcher_eoa_info(b.header.eoa).unwrap() else {
                        return false
                    };
                    if eoa.fund.is_none() {
                        let Some(mev_contract) = b.header.mev_contract else { return false };
                        let Some(contract) =
                            db.try_fetch_searcher_contract_info(mev_contract).unwrap()
                        else {
                            return false
                        };
                        !contract.fund.is_none()
                    } else {
                        true
                    }
                })
                .max_by(|a, b| a.header.profit_usd.total_cmp(&b.header.profit_usd))
                .map(|h| h.header.eoa)
                .unwrap_or_default(),
            arb_top_searcher: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::AtomicArb)
                .max_by(|a, b| a.header.profit_usd.total_cmp(&b.header.profit_usd))
                .map(|r| r.header.eoa)
                .unwrap_or_default(),
            arb_total_profit: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::AtomicArb)
                .map(|b| b.header.profit_usd)
                .sum::<f64>(),
            arb_total_revenue: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::AtomicArb)
                .map(|b| b.header.profit_usd + b.header.bribe_usd)
                .sum::<f64>(),
            arb_unique_searchers: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::AtomicArb)
                .map(|b| b.header.eoa)
                .unique()
                .count() as u64,
            arb_unique_funds: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::AtomicArb)
                .filter_map(|b| {
                    let eoa = db.try_fetch_searcher_eoa_info(b.header.eoa).unwrap()?;
                    if eoa.fund.is_none() {
                        let contract = db
                            .try_fetch_searcher_contract_info(b.header.mev_contract?)
                            .unwrap()?;
                        (!contract.fund.is_none()).then_some(contract.fund)
                    } else {
                        Some(eoa.fund)
                    }
                })
                .unique()
                .count() as u64,
            most_arbed_pair: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::AtomicArb)
                .flat_map(|b| {
                    let BundleData::AtomicArb(arb) = &b.data else { unreachable!() };
                    arb.swaps
                        .iter()
                        .map(|s| Pair(s.token_in.address, s.token_out.address).ordered())
                })
                .counts()
                .iter()
                .max_by_key(|k| k.1)
                .map(|r| *r.0)
                .unwrap_or_default(),
            most_arbed_pool: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::AtomicArb)
                .flat_map(|b| {
                    let BundleData::AtomicArb(arb) = &b.data else { unreachable!() };
                    arb.swaps.iter().map(|s| s.pool)
                })
                .counts()
                .iter()
                .max_by_key(|k| k.1)
                .map(|r| *r.0)
                .unwrap_or_default(),
            most_arbed_dex: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::AtomicArb)
                .flat_map(|b| {
                    let BundleData::AtomicArb(arb) = &b.data else { unreachable!() };
                    arb.swaps.iter().map(|s| s.protocol)
                })
                .counts()
                .iter()
                .max_by_key(|k| k.1)
                .map(|r| *r.0)
                .unwrap_or_default(),
            most_sandwiched_pair: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::Sandwich)
                .flat_map(|b| {
                    let BundleData::Sandwich(sando) = &b.data else { unreachable!() };
                    sando
                        .victim_swaps
                        .iter()
                        .flatten()
                        .map(|s| Pair(s.token_in.address, s.token_out.address).ordered())
                })
                .counts()
                .iter()
                .max_by_key(|k| k.1)
                .map(|r| *r.0)
                .unwrap_or_default(),
            most_sandwiched_pool: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::Sandwich)
                .flat_map(|b| {
                    let BundleData::Sandwich(sando) = &b.data else { unreachable!() };
                    sando.victim_swaps.iter().flatten().map(|s| s.pool)
                })
                .counts()
                .iter()
                .max_by_key(|k| k.1)
                .map(|r| *r.0)
                .unwrap_or_default(),
            most_sandwiched_dex: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::Sandwich)
                .flat_map(|b| {
                    let BundleData::Sandwich(sando) = &b.data else { unreachable!() };
                    sando.victim_swaps.iter().flatten().map(|s| s.protocol)
                })
                .counts()
                .iter()
                .max_by_key(|k| k.1)
                .map(|r| *r.0)
                .unwrap_or_default(),
            sandwich_top_searcher: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::Sandwich)
                .max_by(|a, b| a.header.profit_usd.total_cmp(&b.header.profit_usd))
                .map(|r| r.header.eoa)
                .unwrap_or_default(),
            sandwich_unique_searchers: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::Sandwich)
                .map(|b| b.header.eoa)
                .unique()
                .count() as u64,
            sandwich_total_swapper_loss: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::Sandwich)
                .map(|b| b.header.profit_usd + b.header.bribe_usd)
                .sum::<f64>(),
            sandwich_total_profit: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::Sandwich)
                .map(|b| b.header.profit_usd)
                .sum::<f64>(),
            sandwich_total_revenue: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::Sandwich)
                .map(|b| b.header.profit_usd + b.header.bribe_usd)
                .sum::<f64>(),
            jit_sandwich_total_profit: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::JitSandwich)
                .map(|b| b.header.profit_usd)
                .sum::<f64>(),
            jit_sandwich_total_revenue: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::JitSandwich)
                .map(|b| b.header.profit_usd + b.header.bribe_usd)
                .sum::<f64>(),
            jit_sandwich_top_searcher: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::JitSandwich)
                .max_by(|a, b| a.header.profit_usd.total_cmp(&b.header.profit_usd))
                .map(|r| r.header.eoa)
                .unwrap_or_default(),
            jit_sandwich_total_swapper_loss: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::JitSandwich)
                .map(|b| b.header.profit_usd + b.header.bribe_usd)
                .sum::<f64>(),
            jit_sandwich_unique_searchers: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::JitSandwich)
                .map(|f| f.header.eoa)
                .unique()
                .count() as u64,
            most_jit_sandwiched_pair: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::JitSandwich)
                .flat_map(|b| {
                    let BundleData::JitSandwich(jit_sand) = &b.data else { unreachable!() };
                    jit_sand
                        .victim_swaps
                        .iter()
                        .flatten()
                        .map(|s| Pair(s.token_out.address, s.token_in.address).ordered())
                })
                .counts()
                .iter()
                .max_by_key(|k| k.1)
                .map(|r| *r.0)
                .unwrap_or_default(),
            most_jit_sandwiched_dex: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::JitSandwich)
                .flat_map(|b| {
                    let BundleData::JitSandwich(jit_sand) = &b.data else { unreachable!() };
                    jit_sand.victim_swaps.iter().flatten().map(|s| s.protocol)
                })
                .counts()
                .iter()
                .max_by_key(|k| k.1)
                .map(|r| *r.0)
                .unwrap_or_default(),
            most_jit_sandwiched_pool: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::JitSandwich)
                .flat_map(|b| {
                    let BundleData::JitSandwich(jit_sand) = &b.data else { unreachable!() };
                    jit_sand.victim_swaps.iter().flatten().map(|s| s.pool)
                })
                .counts()
                .iter()
                .max_by_key(|k| k.1)
                .map(|r| *r.0)
                .unwrap_or_default(),
            jit_top_searcher: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::Jit)
                .max_by(|a, b| a.header.profit_usd.total_cmp(&b.header.profit_usd))
                .map(|r| r.header.eoa)
                .unwrap_or_default(),
            jit_total_revenue: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::Jit)
                .map(|b| b.header.profit_usd + b.header.bribe_usd)
                .sum::<f64>(),
            jit_total_profit: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::Jit)
                .map(|b| b.header.profit_usd)
                .sum::<f64>(),
            most_jit_pool: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::Jit)
                .flat_map(|b| {
                    let BundleData::Jit(jit) = &b.data else { unreachable!() };
                    jit.victim_swaps.iter().flatten().map(|s| s.pool)
                })
                .counts()
                .iter()
                .max_by_key(|k| k.1)
                .map(|r| *r.0)
                .unwrap_or_default(),
            most_jit_pair: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::Jit)
                .flat_map(|b| {
                    let BundleData::Jit(jit) = &b.data else { unreachable!() };
                    jit.victim_swaps
                        .iter()
                        .flatten()
                        .map(|s| Pair(s.token_out.address, s.token_in.address).ordered())
                })
                .counts()
                .iter()
                .max_by_key(|k| k.1)
                .map(|r| *r.0)
                .unwrap_or_default(),
            most_jit_dex: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::Jit)
                .flat_map(|b| {
                    let BundleData::Jit(jit) = &b.data else { unreachable!() };
                    jit.victim_swaps.iter().flatten().map(|s| s.protocol)
                })
                .counts()
                .iter()
                .max_by_key(|k| k.1)
                .map(|r| *r.0)
                .unwrap_or_default(),
            jit_unique_searchers: bundles
                .iter()
                .filter(|f| f.data.mev_type() == MevType::Jit)
                .map(|b| b.header.eoa)
                .unique()
                .count() as u64,
            cex_top_fund: bundles
                .iter()
                .filter(|b| b.data.mev_type() == MevType::CexDex)
                .filter(|b| {
                    let Some(eoa) = db.try_fetch_searcher_eoa_info(b.header.eoa).unwrap() else {
                        return false
                    };
                    if eoa.fund.is_none() {
                        let Some(mev_contract) = b.header.mev_contract else { return false };
                        let Some(contract) =
                            db.try_fetch_searcher_contract_info(mev_contract).unwrap()
                        else {
                            return false
                        };
                        !contract.fund.is_none()
                    } else {
                        true
                    }
                })
                .max_by(|a, b| a.header.profit_usd.total_cmp(&b.header.profit_usd))
                .map(|h| h.header.eoa)
                .unwrap_or_default(),
            cex_dex_total_profit: bundles
                .iter()
                .filter(|b| b.data.mev_type() == MevType::CexDex)
                .map(|b| b.header.profit_usd)
                .sum::<f64>(),
            cex_dex_total_rev: bundles
                .iter()
                .filter(|b| b.data.mev_type() == MevType::CexDex)
                .map(|b| b.header.profit_usd + b.header.bribe_usd)
                .sum::<f64>(),
            cex_dex_most_arb_pool_rev: bundles
                .iter()
                .filter(|b| b.data.mev_type() == MevType::CexDex)
                .flat_map(|b| {
                    let BundleData::CexDex(cex) = &b.data else { unreachable!() };
                    cex.optimistic_route_details
                        .iter()
                        .zip(cex.swaps.iter())
                        .map(|(route, pool)| {
                            (pool.pool, route.pnl_pre_gas.maker_taker_mid.0.clone())
                        })
                })
                .into_group_map()
                .iter()
                .max_by_key(|a| a.1.iter().sum::<Rational>())
                .map(|a| *a.0)
                .unwrap_or_default(),
            cex_dex_most_arb_pool_profit: bundles
                .iter()
                .filter(|b| b.data.mev_type() == MevType::CexDex)
                .flat_map(|b| {
                    let BundleData::CexDex(cex) = &b.data else { unreachable!() };
                    cex.optimistic_route_details
                        .iter()
                        .zip(cex.swaps.iter())
                        .map(|(route, pool)| {
                            (
                                pool.pool,
                                &route.pnl_pre_gas.maker_taker_mid.0
                                    - Rational::try_from(b.header.bribe_usd).unwrap(),
                            )
                        })
                })
                .into_group_map()
                .iter()
                .max_by_key(|a| a.1.iter().sum::<Rational>())
                .map(|a| *a.0)
                .unwrap_or_default(),
            cex_dex_most_arb_pair_rev: bundles
                .iter()
                .filter(|b| b.data.mev_type() == MevType::CexDex)
                .flat_map(|b| {
                    let BundleData::CexDex(cex) = &b.data else { unreachable!() };
                    cex.optimistic_route_details
                        .iter()
                        .zip(cex.swaps.iter())
                        .map(|(route, pool)| {
                            (
                                Pair(pool.token_in.address, pool.token_out.address).ordered(),
                                route.pnl_pre_gas.maker_taker_mid.0.clone(),
                            )
                        })
                })
                .into_group_map()
                .iter()
                .max_by_key(|a| a.1.iter().sum::<Rational>())
                .map(|a| *a.0)
                .unwrap_or_default(),
            cex_dex_most_arb_pair_profit: bundles
                .iter()
                .filter(|b| b.data.mev_type() == MevType::CexDex)
                .flat_map(|b| {
                    let BundleData::CexDex(cex) = &b.data else { unreachable!() };
                    cex.optimistic_route_details
                        .iter()
                        .zip(cex.swaps.iter())
                        .map(|(route, pool)| {
                            (
                                Pair(pool.token_in.address, pool.token_out.address).ordered(),
                                &route.pnl_pre_gas.maker_taker_mid.0
                                    - Rational::try_from(b.header.bribe_usd).unwrap(),
                            )
                        })
                })
                .into_group_map()
                .iter()
                .max_by_key(|a| a.1.iter().sum::<Rational>())
                .map(|a| *a.0)
                .unwrap_or_default(),
            cex_top_searcher: bundles
                .iter()
                .filter(|b| b.data.mev_type() == MevType::CexDex)
                .max_by(|a, b| a.header.profit_usd.total_cmp(&b.header.profit_usd))
                .map(|r| r.header.eoa)
                .unwrap_or_default(),
            liquidation_top_searcher: bundles
                .iter()
                .filter(|b| b.data.mev_type() == MevType::Liquidation)
                .max_by(|a, b| a.header.profit_usd.total_cmp(&b.header.profit_usd))
                .map(|r| r.header.eoa)
                .unwrap_or_default(),
            liquidation_average_profit_margin: bundles
                .iter()
                .filter(|b| b.data.mev_type() == MevType::Liquidation && b.header.bribe_usd != 0.0)
                .map(|s| s.header.profit_usd / s.header.bribe_usd)
                .sum::<f64>()
                / Some(
                    bundles
                        .iter()
                        .filter(|b| {
                            b.data.mev_type() == MevType::Liquidation && b.header.bribe_usd != 0.0
                        })
                        .count(),
                )
                .filter(|value| *value != 0)
                .map(|f| f as f64)
                .unwrap_or(1.0),
            liquidation_total_revenue: bundles
                .iter()
                .filter(|b| b.data.mev_type() == MevType::Liquidation)
                .map(|s| s.header.profit_usd + s.header.bribe_usd)
                .sum::<f64>(),
            liquidation_total_profit: bundles
                .iter()
                .filter(|b| b.data.mev_type() == MevType::Liquidation)
                .map(|s| s.header.profit_usd)
                .sum::<f64>(),
            total_usd_liquidated: bundles
                .iter()
                .filter(|b| b.data.mev_type() == MevType::Liquidation)
                .map(|s| s.header.profit_usd + s.header.bribe_usd)
                .sum::<f64>(),
            most_liquidated_token: bundles
                .iter()
                .filter(|b| b.data.mev_type() == MevType::Liquidation)
                .flat_map(|b| {
                    let BundleData::Liquidation(liq) = &b.data else { unreachable!() };
                    liq.liquidations.iter().map(|l| l.collateral_asset.address)
                })
                .counts()
                .iter()
                .max_by_key(|k| k.1)
                .map(|r| *r.0)
                .unwrap_or_default(),
            most_liquidated_protocol: bundles
                .iter()
                .filter(|b| b.data.mev_type() == MevType::Liquidation)
                .flat_map(|b| {
                    let BundleData::Liquidation(liq) = &b.data else { unreachable!() };
                    liq.liquidations.iter().map(|l| l.protocol)
                })
                .counts()
                .iter()
                .max_by_key(|k| k.1)
                .map(|r| *r.0)
                .unwrap_or_default(),
            liquidation_unique_searchers: bundles
                .iter()
                .filter(|b| b.data.mev_type() == MevType::Liquidation)
                .map(|b| b.header.eoa)
                .unique()
                .count() as u64,
        }
    }
}
