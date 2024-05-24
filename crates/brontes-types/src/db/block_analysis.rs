use std::hash::Hash;

use alloy_primitives::Address;
use clickhouse::Row;
use itertools::Itertools;
use malachite::Rational;
use reth_primitives::TxHash;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};

use super::traits::LibmdbxReader;
use crate::{
    db::{
        clickhouse_serde::pair::{addr_ser, pair_ser},
        searcher::Fund,
    },
    mev::{Bundle, BundleData, Mev, MevBlock, MevType},
    pair::Pair,
    Protocol,
};

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, Row)]
pub struct BlockAnalysis {
    pub block_number: u64,

    // all
    pub all_total_profit:          f64,
    pub all_total_revenue:         f64,
    pub all_average_profit_margin: f64,

    pub all_top_searcher_rev:         f64,
    pub all_top_searcher_rev_addr:    Address,
    pub all_top_searcher_profit:      f64,
    pub all_top_searcher_profit_addr: Address,
    pub all_searchers:                u64,

    pub all_top_fund_rev:       f64,
    pub all_top_fund_rev_id:    Fund,
    pub all_top_fund_profit:    f64,
    pub all_top_fund_profit_id: Fund,
    pub all_fund_count:         u64,

    pub all_most_arbed_pool_profit_address: Address,
    pub all_most_arbed_pool_profit:         f64,

    pub all_most_arbed_pool_revenue_address: Address,
    pub all_most_arbed_pool_revenue:         f64,

    pub all_most_arbed_pair_profit_address: Pair,
    pub all_most_arbed_pair_profit:         f64,

    pub all_most_arbed_pair_revenue_address: Pair,
    pub all_most_arbed_pair_revenue:         f64,

    // atomic
    pub atomic_total_profit: f64,
    pub atomic_total_revenue: f64,
    pub atomic_average_profit_margin: f64,
    pub atomic_top_searcher_rev: f64,
    pub atomic_top_searcher_rev_addr: Address,
    pub atomic_top_searcher_profit: f64,
    pub atomic_top_searcher_profit_addr: Address,
    pub atomic_searchers: u64,
    pub atomic_top_fund_rev: f64,
    pub atomic_top_fund_rev_id: Fund,
    pub atomic_top_fund_profit: f64,
    pub atomic_top_fund_profit_id: Fund,
    pub atomic_fund_count: u64,
    pub atomic_most_arbed_pool_profit_address: Address,
    pub atomic_most_arbed_pool_profit: f64,
    pub atomic_most_arbed_pool_revenue_address: Address,
    pub atomic_most_arbed_pool_revenue: f64,
    pub atomic_most_arbed_pair_profit_address: Pair,
    pub atomic_most_arbed_pair_profit: f64,
    pub atomic_most_arbed_pair_revenue_address: Pair,
    pub atomic_most_arbed_pair_revenue: f64,
    pub atomic_most_arbed_dex_profit_address: Protocol,
    pub atomic_most_arbed_dex_profit: f64,
    pub atomic_most_arbed_dex_revenue_address: Protocol,
    pub atomic_most_arbed_dex_revenue: f64,

    // sandwich
    pub sandwich_total_profit: f64,
    pub sandwich_total_revenue: f64,
    pub sandwich_average_profit_margin: f64,
    pub sandwich_top_searcher_rev: f64,
    pub sandwich_top_searcher_rev_addr: Address,
    pub sandwich_top_searcher_profit: f64,
    pub sandwich_top_searcher_profit_addr: Address,
    pub sandwich_searchers: u64,
    pub sandwich_most_arbed_pool_profit_address: Address,
    pub sandwich_most_arbed_pool_profit: f64,
    pub sandwich_most_arbed_pool_revenue_address: Address,
    pub sandwich_most_arbed_pool_revenue: f64,
    pub sandwich_most_arbed_pair_profit_address: Pair,
    pub sandwich_most_arbed_pair_profit: f64,
    pub sandwich_most_arbed_pair_revenue_address: Pair,
    pub sandwich_most_arbed_pair_revenue: f64,
    pub sandwich_most_arbed_dex_profit_address: Protocol,
    pub sandwich_most_arbed_dex_profit: f64,
    pub sandwich_most_arbed_dex_revenue_address: Protocol,
    pub sandwich_most_arbed_dex_revenue: f64,
    pub sandwich_biggest_arb_profit_hash: TxHash,
    pub sandwich_biggest_arb_profit: f64,
    pub sandwich_biggest_arb_revenue_hash: TxHash,
    pub sandwich_biggest_arb_revenue: f64,

    // jit
    pub jit_total_profit: f64,
    pub jit_total_revenue: f64,
    pub jit_average_profit_margin: f64,
    pub jit_top_searcher_rev: f64,
    pub jit_top_searcher_rev_addr: Address,
    pub jit_top_searcher_profit: f64,
    pub jit_top_searcher_profit_addr: Address,
    pub jit_searchers: u64,
    pub jit_most_arbed_pool_profit_address: Address,
    pub jit_most_arbed_pool_profit: f64,
    pub jit_most_arbed_pool_revenue_address: Address,
    pub jit_most_arbed_pool_revenue: f64,
    pub jit_most_arbed_pair_profit_address: Pair,
    pub jit_most_arbed_pair_profit: f64,
    pub jit_most_arbed_pair_revenue_address: Pair,
    pub jit_most_arbed_pair_revenue: f64,
    pub jit_most_arbed_dex_profit_address: Protocol,
    pub jit_most_arbed_dex_profit: f64,
    pub jit_most_arbed_dex_revenue_address: Protocol,
    pub jit_most_arbed_dex_revenue: f64,

    // jit-sandwich
    pub jit_sandwich_total_profit: f64,
    pub jit_sandwich_total_revenue: f64,
    pub jit_sandwich_average_profit_margin: f64,
    pub jit_sandwich_top_searcher_rev: f64,
    pub jit_sandwich_top_searcher_rev_addr: Address,
    pub jit_sandwich_top_searcher_profit: f64,
    pub jit_sandwich_top_searcher_profit_addr: Address,
    pub jit_sandwich_searchers: u64,
    pub jit_sandwich_most_arbed_pool_profit_address: Address,
    pub jit_sandwich_most_arbed_pool_profit: f64,
    pub jit_sandwich_most_arbed_pool_revenue_address: Address,
    pub jit_sandwich_most_arbed_pool_revenue: f64,
    pub jit_sandwich_most_arbed_pair_profit_address: Pair,
    pub jit_sandwich_most_arbed_pair_profit: f64,
    pub jit_sandwich_most_arbed_pair_revenue_address: Pair,
    pub jit_sandwich_most_arbed_pair_revenue: f64,
    pub jit_sandwich_most_arbed_dex_profit_address: Protocol,
    pub jit_sandwich_most_arbed_dex_profit: f64,
    pub jit_sandwich_most_arbed_dex_revenue_address: Protocol,
    pub jit_sandwich_most_arbed_dex_revenue: f64,
    pub jit_sandwich_biggest_arb_profit_hash: TxHash,
    pub jit_sandwich_biggest_arb_profit: f64,
    pub jit_sandwich_biggest_arb_revenue_hash: TxHash,
    pub jit_sandwich_biggest_arb_revenue: f64,

    // cex dex
    pub cex_dex_total_profit: f64,
    pub cex_dex_total_revenue: f64,
    pub cex_dex_average_profit_margin: f64,
    pub cex_dex_top_searcher_rev: f64,
    pub cex_dex_top_searcher_rev_addr: Address,
    pub cex_dex_top_searcher_profit: f64,
    pub cex_dex_top_searcher_profit_addr: Address,
    pub cex_dex_searchers: u64,
    pub cex_dex_top_fund_rev: f64,
    pub cex_dex_top_fund_rev_id: Fund,
    pub cex_dex_top_fund_profit: f64,
    pub cex_dex_top_fund_profit_id: Fund,
    pub cex_dex_fund_count: u64,
    pub cex_dex_most_arbed_pool_profit_address: Address,
    pub cex_dex_most_arbed_pool_profit: f64,
    pub cex_dex_most_arbed_pool_revenue_address: Address,
    pub cex_dex_most_arbed_pool_revenue: f64,
    pub cex_dex_most_arbed_pair_profit_address: Pair,
    pub cex_dex_most_arbed_pair_profit: f64,
    pub cex_dex_most_arbed_pair_revenue_address: Pair,
    pub cex_dex_most_arbed_pair_revenue: f64,
    pub cex_dex_most_arbed_dex_profit_address: Protocol,
    pub cex_dex_most_arbed_dex_profit: f64,
    pub cex_dex_most_arbed_dex_revenue_address: Protocol,
    pub cex_dex_most_arbed_dex_revenue: f64,

    // liquidation
    pub liquidation_total_profit:             f64,
    pub liquidation_total_revenue:            f64,
    pub liquidation_average_profit_margin:    f64,
    pub liquidation_top_searcher_rev:         f64,
    pub liquidation_top_searcher_rev_addr:    Address,
    pub liquidation_top_searcher_profit:      f64,
    pub liquidation_top_searcher_profit_addr: Address,
    pub liquidation_searchers:                u64,

    pub most_liquidated_token_rev_address:    Address,
    pub most_liquidated_token_rev:            f64,
    pub most_liquidated_token_profit_address: Address,
    pub most_liquidated_token_profit:         f64,
    pub total_usd_liquidated:                 f64,
}

impl BlockAnalysis {
    pub fn new<DB: LibmdbxReader>(block: &MevBlock, bundles: &[Bundle], db: DB) -> Self {
        todo!()
    }

    fn total_revenue_by_type(mev_type: MevType, bundles: &[Bundle]) -> f64 {
        bundles
            .iter()
            .filter(|b| b.data.mev_type() == mev_type)
            .map(|s| s.header.profit_usd + s.header.bribe_usd)
            .sum::<f64>()
    }

    fn total_profit_by_type(mev_type: MevType, bundles: &[Bundle]) -> f64 {
        bundles
            .iter()
            .filter(|b| b.data.mev_type() == mev_type)
            .map(|s| s.header.profit_usd)
            .sum::<f64>()
    }

    fn top_fund_by_type_profit<DB: LibmdbxReader>(
        mev_type: MevType,
        bundles: &[Bundle],
        db: &DB,
    ) -> Option<(Fund, f64)> {
        bundles
            .iter()
            .filter_map(|b| {
                if b.data.mev_type() != mev_type {
                    return None
                }

                let Some(eoa) = db.try_fetch_searcher_eoa_info(b.header.eoa).unwrap() else {
                    return None
                };
                if eoa.fund.is_none() {
                    let Some(mev_contract) = b.header.mev_contract else { return None };
                    let Some(contract) = db.try_fetch_searcher_contract_info(mev_contract).unwrap()
                    else {
                        return None
                    };
                    Some((contract.fund, b.header.profit_usd))
                } else {
                    Some((eoa.fund, b.header.profit_usd))
                }
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
    }

    fn top_fund_by_type_rev<DB: LibmdbxReader>(
        mev_type: MevType,
        bundles: &[Bundle],
        db: &DB,
    ) -> Option<(Fund, f64)> {
        bundles
            .iter()
            .filter_map(|b| {
                if b.data.mev_type() != mev_type {
                    return None
                }

                let Some(eoa) = db.try_fetch_searcher_eoa_info(b.header.eoa).unwrap() else {
                    return None
                };
                if eoa.fund.is_none() {
                    let Some(mev_contract) = b.header.mev_contract else { return None };
                    let Some(contract) = db.try_fetch_searcher_contract_info(mev_contract).unwrap()
                    else {
                        return None
                    };
                    Some((contract.fund, b.header.profit_usd + b.header.bribe_usd))
                } else {
                    Some((eoa.fund, b.header.profit_usd + b.header.bribe_usd))
                }
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
    }

    fn top_searcher_by_profit(mev_type: MevType, bundles: &[Bundle]) -> Option<(Address, f64)> {
        bundles
            .iter()
            .filter(|b| b.data.mev_type() == mev_type)
            .max_by(|a, b| a.header.profit_usd.total_cmp(&b.header.profit_usd))
            .map(|r| (r.header.eoa, r.header.profit_usd))
    }

    fn top_searcher_by_rev(mev_type: MevType, bundles: &[Bundle]) -> Option<(Address, f64)> {
        bundles
            .iter()
            .filter(|b| b.data.mev_type() == mev_type)
            .max_by(|a, b| {
                (a.header.profit_usd + a.header.bribe_usd)
                    .total_cmp(&(b.header.profit_usd + b.header.bribe_usd))
            })
            .map(|r| (r.header.eoa, r.header.profit_usd + r.header.bribe_usd))
    }

    fn most_transacted_pool_profit(
        mev_type: MevType,
        bundles: &[Bundle],
        f: impl Fn(&BundleData) -> Vec<Address>,
    ) -> Option<(Address, f64)> {
        Self::most_transacted_profit(mev_type, bundles, f)
    }

    fn most_transacted_pool_rev(
        mev_type: MevType,
        bundles: &[Bundle],
        f: impl Fn(&BundleData) -> Vec<Address>,
    ) -> Option<(Address, f64)> {
        Self::most_transacted_rev(mev_type, bundles, f)
    }

    fn most_transacted_pair_profit(
        mev_type: MevType,
        bundles: &[Bundle],
        f: impl Fn(&BundleData) -> Vec<Pair>,
    ) -> Option<(Pair, f64)> {
        Self::most_transacted_profit(mev_type, bundles, f)
    }

    fn most_transacted_pair_rev(
        mev_type: MevType,
        bundles: &[Bundle],
        f: impl Fn(&BundleData) -> Vec<Pair>,
    ) -> Option<(Pair, f64)> {
        Self::most_transacted_rev(mev_type, bundles, f)
    }

    fn most_transacted_dex_profit(
        mev_type: MevType,
        bundles: &[Bundle],
        f: impl Fn(&BundleData) -> Vec<Protocol>,
    ) -> Option<(Protocol, f64)> {
        Self::most_transacted_profit(mev_type, bundles, f)
    }

    fn most_transacted_dex_rev(
        mev_type: MevType,
        bundles: &[Bundle],
        f: impl Fn(&BundleData) -> Vec<Protocol>,
    ) -> Option<(Protocol, f64)> {
        Self::most_transacted_rev(mev_type, bundles, f)
    }

    fn average_profit_margin(mev_type: MevType, bundles: &[Bundle]) -> Option<f64> {
        Some(
            bundles
                .iter()
                .filter(|b| b.data.mev_type() == mev_type && b.header.bribe_usd != 0.0)
                .map(|s| s.header.profit_usd / s.header.bribe_usd)
                .sum::<f64>()
                / Some(
                    bundles
                        .iter()
                        .filter(|b| b.data.mev_type() == mev_type && b.header.bribe_usd != 0.0)
                        .count(),
                )
                .filter(|value| *value != 0)
                .map(|f| f as f64)?,
        )
    }

    fn unique(mev_type: MevType, bundles: &[Bundle]) -> u64 {
        bundles
            .iter()
            .filter(|b| b.data.mev_type() == mev_type)
            .map(|b| b.header.eoa)
            .unique()
            .count() as u64
    }

    fn most_transacted_profit<Ty: Copy + Hash + Eq>(
        mev_type: MevType,
        bundles: &[Bundle],
        f: impl Fn(&BundleData) -> Vec<Ty>,
    ) -> Option<(Ty, f64)> {
        bundles
            .iter()
            .filter(|b| b.data.mev_type() == mev_type)
            .flat_map(|b| {
                let res = f(&b.data);
                let mut merged = Vec::with_capacity(res.len());
                for r in res {
                    merged.push((r, b.header.profit_usd));
                }
                merged
            })
            .into_group_map()
            .iter()
            .max_by(|x, y| x.1.iter().sum::<f64>().total_cmp(&y.1.iter().sum::<f64>()))
            .map(|r| (*r.0, r.1.iter().sum::<f64>()))
    }

    fn most_transacted_rev<Ty: Copy + Hash + Eq>(
        mev_type: MevType,
        bundles: &[Bundle],
        f: impl Fn(&BundleData) -> Vec<Ty>,
    ) -> Option<(Ty, f64)> {
        bundles
            .iter()
            .filter(|b| b.data.mev_type() == mev_type)
            .flat_map(|b| {
                let res = f(&b.data);
                let mut merged = Vec::with_capacity(res.len());
                for r in res {
                    merged.push((r, b.header.profit_usd + b.header.bribe_usd));
                }
                merged
            })
            .into_group_map()
            .iter()
            .max_by(|x, y| x.1.iter().sum::<f64>().total_cmp(&y.1.iter().sum::<f64>()))
            .map(|r| (*r.0, r.1.iter().sum::<f64>()))
    }
}
