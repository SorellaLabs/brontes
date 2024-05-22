use alloy_primitives::Address;

use crate::{pair::Pair, Protocol};

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
    pub most_sandwiched_pair:        Pair,
    pub most_sandwiched_pool:        Address,
    pub most_sandwiched_dex:         Protocol,
    pub sandwich_total_revenue:      f64,
    pub sandwich_total_profit:       f64,
    pub sandwich_total_swapper_loss: f64,
    pub sandwich_top_searcher:       Address,
    pub sandwich_unique_searchers:   u64,

    // jit
    pub most_jit_pair:        Pair,
    pub most_jit_pool:        Address,
    pub most_jit_dex:         Protocol,
    pub jit_total_revenue:    f64,
    pub jit_total_profit:     f64,
    pub jit_top_searcher:     Address,
    pub jit_unique_searchers: u64,

    // jit-sandwich
    pub most_jit_sandwiched_pair:        Pair,
    pub most_jit_sandwiched_pool:        Address,
    pub most_jit_sandwiched_dex:         Protocol,
    pub jit_sandwich_total_revenue:      f64,
    pub jit_sandwich_total_profit:       f64,
    pub jit_sandwich_total_swapper_loss: f64,
    pub jit_sandwich_top_searcher:       Address,
    pub jit_sandwich_unique_searchers:   u64,

    // cex dex
    pub cex_dex_most_arb_pair_rev:    Pair,
    pub cex_dex_most_arb_pool_rev:    Address,
    pub cex_dex_most_arb_pair_profit: Pair,
    pub cex_dex_most_arb_pool_profit: Address,
    pub cex_dex_total_rev:            f64,
    pub cex_dex_total_profit:         f64,
    pub cex_top_searcher:             Address,
    pub cex_top_fund:                 Address,

    // liquidation
    pub most_liquidated_token:    Address,
    pub most_liquidated_protocol: Protocol,
    pub total_revenue:            f64,
    pub total_profit:             f64,
    pub average_profit_margin:    f64,
    pub top_searcher:             Address,
    pub unique_searchers:         u64,
    pub total_usd_liquidated:     f64,
}
