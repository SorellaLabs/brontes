use alloy_primitives::Address;
use clickhouse::Row;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use super::traits::LibmdbxReader;
use crate::{
    db::clickhouse_serde::pair::pair_ser,
    mev::{Bundle, MevBlock},
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
    pub liqudiation_top_searcher:          Address,
    pub liqudation_unique_searchers:       u64,
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
                        return (!contract.fund.is_none()).then_some(contract.fund)
                    } else {
                        return Some(eoa.fund)
                    }
                })
                .unique()
                .count() as u64,
            all_unique_searchers: bundles.iter().map(|b| b.header.eoa).unique().count() as u64,
            all_top_fund:         bundles
                .iter()
                .filter(|b| {
                    let eoa = db.try_fetch_searcher_eoa_info(b.header.eoa).unwrap()?;
                    if eoa.fund.is_none() {
                        let contract = db
                            .try_fetch_searcher_contract_info(b.header.mev_contract?)
                            .unwrap()?;
                        if contract.fund.is_none() {
                            false
                        } else {
                            true
                        }
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
                / bundles.len() as f64,

        }
    }
}
