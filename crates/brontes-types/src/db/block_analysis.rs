use std::hash::Hash;

use alloy_primitives::Address;
use clickhouse::Row;
use itertools::Itertools;
use reth_primitives::TxHash;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::{
    db::searcher::Fund,
    mev::{Bundle, BundleData, Mev, MevBlock, MevType},
    pair::Pair,
    serde_utils::{option_address, option_fund, option_pair, option_protocol, option_txhash},
    Protocol,
};

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, Row, Default)]
pub struct BlockAnalysis {
    pub block_number:              u64,
    // all
    pub all_total_profit:          f64,
    pub all_total_revenue:         f64,
    pub all_average_profit_margin: f64,

    pub all_top_searcher_rev:         Option<f64>,
    #[serde(with = "option_address")]
    pub all_top_searcher_rev_addr:    Option<Address>,
    pub all_top_searcher_profit:      Option<f64>,
    #[serde(with = "option_address")]
    pub all_top_searcher_profit_addr: Option<Address>,
    pub all_searchers:                u64,

    pub all_top_fund_rev:       Option<f64>,
    #[serde(with = "option_fund")]
    pub all_top_fund_rev_id:    Option<Fund>,
    pub all_top_fund_profit:    Option<f64>,
    #[serde(with = "option_fund")]
    pub all_top_fund_profit_id: Option<Fund>,
    pub all_fund_count:         u64,

    #[serde(with = "option_address")]
    pub all_most_arbed_pool_address: Option<Address>,
    pub all_most_arbed_pool_profit:  Option<f64>,
    pub all_most_arbed_pool_revenue: Option<f64>,

    #[serde(with = "option_pair")]
    pub all_most_arbed_pair_address: Option<Pair>,
    pub all_most_arbed_pair_profit:  Option<f64>,
    pub all_most_arbed_pair_revenue: Option<f64>,

    // atomic
    pub atomic_total_profit:             f64,
    pub atomic_total_revenue:            f64,
    pub atomic_average_profit_margin:    f64,
    pub atomic_top_searcher_rev:         Option<f64>,
    #[serde(with = "option_address")]
    pub atomic_top_searcher_rev_addr:    Option<Address>,
    pub atomic_top_searcher_profit:      Option<f64>,
    #[serde(with = "option_address")]
    pub atomic_top_searcher_profit_addr: Option<Address>,
    pub atomic_searchers:                u64,
    pub atomic_top_fund_rev:             Option<f64>,
    #[serde(with = "option_fund")]
    pub atomic_top_fund_rev_id:          Option<Fund>,
    pub atomic_top_fund_profit:          Option<f64>,
    #[serde(with = "option_fund")]
    pub atomic_top_fund_profit_id:       Option<Fund>,
    pub atomic_fund_count:               u64,

    #[serde(with = "option_address")]
    pub atomic_most_arbed_pool_address: Option<Address>,
    pub atomic_most_arbed_pool_profit:  Option<f64>,
    pub atomic_most_arbed_pool_revenue: Option<f64>,

    #[serde(with = "option_pair")]
    pub atomic_most_arbed_pair_address: Option<Pair>,
    pub atomic_most_arbed_pair_profit:  Option<f64>,
    pub atomic_most_arbed_pair_revenue: Option<f64>,

    #[serde(with = "option_protocol")]
    pub atomic_most_arbed_dex_address: Option<Protocol>,
    pub atomic_most_arbed_dex_profit:  Option<f64>,
    pub atomic_most_arbed_dex_revenue: Option<f64>,

    // sandwich
    pub sandwich_total_profit:             f64,
    pub sandwich_total_revenue:            f64,
    pub sandwich_average_profit_margin:    f64,
    pub sandwich_top_searcher_rev:         Option<f64>,
    #[serde(with = "option_address")]
    pub sandwich_top_searcher_rev_addr:    Option<Address>,
    pub sandwich_top_searcher_profit:      Option<f64>,
    #[serde(with = "option_address")]
    pub sandwich_top_searcher_profit_addr: Option<Address>,
    pub sandwich_searchers:                u64,

    #[serde(with = "option_address")]
    pub sandwich_most_arbed_pool_address:  Option<Address>,
    pub sandwich_most_arbed_pool_profit:   Option<f64>,
    pub sandwich_most_arbed_pool_revenue:  Option<f64>,
    #[serde(with = "option_pair")]
    pub sandwich_most_arbed_pair_address:  Option<Pair>,
    pub sandwich_most_arbed_pair_profit:   Option<f64>,
    pub sandwich_most_arbed_pair_revenue:  Option<f64>,
    #[serde(with = "option_protocol")]
    pub sandwich_most_arbed_dex_address:   Option<Protocol>,
    pub sandwich_most_arbed_dex_profit:    Option<f64>,
    pub sandwich_most_arbed_dex_revenue:   Option<f64>,
    #[serde(with = "option_txhash")]
    pub sandwich_biggest_arb_profit_hash:  Option<TxHash>,
    pub sandwich_biggest_arb_profit:       Option<f64>,
    #[serde(with = "option_txhash")]
    pub sandwich_biggest_arb_revenue_hash: Option<TxHash>,
    pub sandwich_biggest_arb_revenue:      Option<f64>,

    // jit
    pub jit_total_profit:             f64,
    pub jit_total_revenue:            f64,
    pub jit_average_profit_margin:    f64,
    pub jit_top_searcher_rev:         Option<f64>,
    #[serde(with = "option_address")]
    pub jit_top_searcher_rev_addr:    Option<Address>,
    pub jit_top_searcher_profit:      Option<f64>,
    #[serde(with = "option_address")]
    pub jit_top_searcher_profit_addr: Option<Address>,
    pub jit_searchers:                u64,
    #[serde(with = "option_address")]
    pub jit_most_arbed_pool_address:  Option<Address>,
    pub jit_most_arbed_pool_profit:   Option<f64>,
    pub jit_most_arbed_pool_revenue:  Option<f64>,
    #[serde(with = "option_pair")]
    pub jit_most_arbed_pair_address:  Option<Pair>,
    pub jit_most_arbed_pair_profit:   Option<f64>,
    pub jit_most_arbed_pair_revenue:  Option<f64>,
    #[serde(with = "option_protocol")]
    pub jit_most_arbed_dex_address:   Option<Protocol>,
    pub jit_most_arbed_dex_profit:    Option<f64>,
    pub jit_most_arbed_dex_revenue:   Option<f64>,

    // jit-sandwich
    pub jit_sandwich_total_profit:             f64,
    pub jit_sandwich_total_revenue:            f64,
    pub jit_sandwich_average_profit_margin:    f64,
    pub jit_sandwich_top_searcher_rev:         Option<f64>,
    #[serde(with = "option_address")]
    pub jit_sandwich_top_searcher_rev_addr:    Option<Address>,
    pub jit_sandwich_top_searcher_profit:      Option<f64>,
    #[serde(with = "option_address")]
    pub jit_sandwich_top_searcher_profit_addr: Option<Address>,
    pub jit_sandwich_searchers:                u64,
    #[serde(with = "option_address")]
    pub jit_sandwich_most_arbed_pool_address:  Option<Address>,
    pub jit_sandwich_most_arbed_pool_profit:   Option<f64>,
    pub jit_sandwich_most_arbed_pool_revenue:  Option<f64>,
    #[serde(with = "option_pair")]
    pub jit_sandwich_most_arbed_pair_address:  Option<Pair>,
    pub jit_sandwich_most_arbed_pair_profit:   Option<f64>,
    pub jit_sandwich_most_arbed_pair_revenue:  Option<f64>,
    #[serde(with = "option_protocol")]
    pub jit_sandwich_most_arbed_dex_address:   Option<Protocol>,
    pub jit_sandwich_most_arbed_dex_profit:    Option<f64>,
    pub jit_sandwich_most_arbed_dex_revenue:   Option<f64>,
    #[serde(with = "option_txhash")]
    pub jit_sandwich_biggest_arb_profit_hash:  Option<TxHash>,
    pub jit_sandwich_biggest_arb_profit:       Option<f64>,
    #[serde(with = "option_txhash")]
    pub jit_sandwich_biggest_arb_revenue_hash: Option<TxHash>,
    pub jit_sandwich_biggest_arb_revenue:      Option<f64>,

    // cex dex
    pub cex_dex_total_profit:             f64,
    pub cex_dex_total_revenue:            f64,
    pub cex_dex_average_profit_margin:    f64,
    pub cex_dex_top_searcher_rev:         Option<f64>,
    #[serde(with = "option_address")]
    pub cex_dex_top_searcher_rev_addr:    Option<Address>,
    pub cex_dex_top_searcher_profit:      Option<f64>,
    #[serde(with = "option_address")]
    pub cex_dex_top_searcher_profit_addr: Option<Address>,
    pub cex_dex_searchers:                u64,
    pub cex_dex_top_fund_rev:             Option<f64>,
    #[serde(with = "option_fund")]
    pub cex_dex_top_fund_rev_id:          Option<Fund>,
    pub cex_dex_top_fund_profit:          Option<f64>,
    #[serde(with = "option_fund")]
    pub cex_dex_top_fund_profit_id:       Option<Fund>,
    pub cex_dex_fund_count:               u64,
    #[serde(with = "option_address")]
    pub cex_dex_most_arbed_pool_address:  Option<Address>,
    pub cex_dex_most_arbed_pool_profit:   Option<f64>,
    pub cex_dex_most_arbed_pool_revenue:  Option<f64>,
    #[serde(with = "option_pair")]
    pub cex_dex_most_arbed_pair_address:  Option<Pair>,
    pub cex_dex_most_arbed_pair_profit:   Option<f64>,
    pub cex_dex_most_arbed_pair_revenue:  Option<f64>,
    #[serde(with = "option_protocol")]
    pub cex_dex_most_arbed_dex_address:   Option<Protocol>,
    pub cex_dex_most_arbed_dex_profit:    Option<f64>,
    pub cex_dex_most_arbed_dex_revenue:   Option<f64>,

    // liquidation
    pub liquidation_total_profit:             f64,
    pub liquidation_total_revenue:            f64,
    pub liquidation_average_profit_margin:    f64,
    pub liquidation_top_searcher_rev:         Option<f64>,
    #[serde(with = "option_address")]
    pub liquidation_top_searcher_rev_addr:    Option<Address>,
    pub liquidation_top_searcher_profit:      Option<f64>,
    #[serde(with = "option_address")]
    pub liquidation_top_searcher_profit_addr: Option<Address>,
    pub liquidation_searchers:                u64,
    #[serde(with = "option_address")]
    pub most_liquidated_token_address:        Option<Address>,
    pub most_liquidated_token_rev:            Option<f64>,
    pub most_liquidated_token_profit:         Option<f64>,
    pub total_usd_liquidated:                 f64,
}

impl BlockAnalysis {
    pub fn new(block: &MevBlock, bundles: &[Bundle]) -> Self {
        // All fields
        let (all_profit_addr, all_profit_am) =
            Self::top_searcher_by_profit(|b| b != MevType::SearcherTx, bundles).unzip();
        let (all_rev_addr, all_rev_am) =
            Self::top_searcher_by_rev(|b| b != MevType::SearcherTx, bundles).unzip();

        let (fund_rev, fund_rev_am) =
            Self::top_fund_by_type_rev(|b| b != MevType::SearcherTx, bundles).unzip();
        let (fund_profit, fund_profit_am) =
            Self::top_fund_by_type_rev(|b| b != MevType::SearcherTx, bundles).unzip();

        let (all_pool, all_pool_prof, all_pool_rev) = Self::most_transacted_pool(
            |b| b != MevType::SearcherTx && b != MevType::Liquidation,
            bundles,
            Self::get_pool_fn,
        )
        .three_unzip();

        let (all_pair, all_pair_prof, all_pair_rev) = Self::most_transacted_pair(
            |b| b != MevType::SearcherTx && b != MevType::Liquidation,
            bundles,
            Self::get_pair_fn,
        )
        .three_unzip();

        // Atomic Fields
        let (atomic_searcher_prof_addr, atomic_searcher_prof) =
            Self::top_searcher_by_profit(|b| b == MevType::AtomicArb, bundles).unzip();
        let (atomic_searcher_rev_addr, atomic_searcher_rev) =
            Self::top_searcher_by_rev(|b| b == MevType::AtomicArb, bundles).unzip();

        let (atomic_fund_rev_addr, atomic_fund_rev) =
            Self::top_fund_by_type_rev(|b| b == MevType::AtomicArb, bundles).unzip();
        let (atomic_fund_profit_addr, atomic_fund_profit) =
            Self::top_fund_by_type_profit(|b| b == MevType::AtomicArb, bundles).unzip();

        let (atomic_pool_addr, atomic_pool_prof, atomic_pool_rev) =
            Self::most_transacted_pool(|b| b == MevType::AtomicArb, bundles, Self::get_pool_fn)
                .three_unzip();
        let (atomic_pair_addr, atomic_pair_prof, atomic_pair_rev) =
            Self::most_transacted_pair(|b| b == MevType::AtomicArb, bundles, Self::get_pair_fn)
                .three_unzip();
        let (atomic_dex_addr, atomic_dex_prof, atomic_dex_rev) =
            Self::most_transacted_dex(|b| b == MevType::AtomicArb, bundles, Self::get_dex_fn)
                .three_unzip();

        // Sandwich Fields
        let (sandwich_biggest_tx_prof, sandwich_biggest_prof) =
            Self::biggest_arb_profit(|b| b == MevType::Sandwich, bundles).unzip();
        let (sandwich_biggest_tx_rev, sandwich_biggest_rev) =
            Self::biggest_arb_revenue(|b| b == MevType::Sandwich, bundles).unzip();

        let (sandwich_searcher_prof_addr, sandwich_searcher_prof) =
            Self::top_searcher_by_profit(|b| b == MevType::Sandwich, bundles).unzip();
        let (sandwich_searcher_rev_addr, sandwich_searcher_rev) =
            Self::top_searcher_by_rev(|b| b == MevType::Sandwich, bundles).unzip();
        let (sandwich_pool_addr, sandwich_pool_prof, sandwich_pool_rev) =
            Self::most_transacted_pool(|b| b == MevType::Sandwich, bundles, Self::get_pool_fn)
                .three_unzip();
        let (sandwich_pair_addr, sandwich_pair_prof, sandwich_pair_rev) =
            Self::most_transacted_pair(|b| b == MevType::Sandwich, bundles, Self::get_pair_fn)
                .three_unzip();
        let (sandwich_dex_addr, sandwich_dex_prof, sandwich_dex_rev) =
            Self::most_transacted_dex(|b| b == MevType::Sandwich, bundles, Self::get_dex_fn)
                .three_unzip();

        // Jit Fields
        let (jit_searcher_prof_addr, jit_searcher_prof) =
            Self::top_searcher_by_profit(|b| b == MevType::Jit, bundles).unzip();
        let (jit_searcher_rev_addr, jit_searcher_rev) =
            Self::top_searcher_by_rev(|b| b == MevType::Jit, bundles).unzip();
        let (jit_pool_addr, jit_pool_prof, jit_pool_rev) =
            Self::most_transacted_pool(|b| b == MevType::Jit, bundles, Self::get_pool_fn)
                .three_unzip();
        let (jit_pair_addr, jit_pair_prof, jit_pair_rev) =
            Self::most_transacted_pair(|b| b == MevType::Jit, bundles, Self::get_pair_fn)
                .three_unzip();
        let (jit_dex_addr, jit_dex_prof, jit_dex_rev) =
            Self::most_transacted_dex(|b| b == MevType::Jit, bundles, Self::get_dex_fn)
                .three_unzip();

        // Jit Sando Fields
        let (jit_sandwich_biggest_tx_prof, jit_sandwich_biggest_prof) =
            Self::biggest_arb_profit(|b| b == MevType::JitSandwich, bundles).unzip();
        let (jit_sandwich_biggest_tx_rev, jit_sandwich_biggest_rev) =
            Self::biggest_arb_revenue(|b| b == MevType::JitSandwich, bundles).unzip();
        let (jit_sandwich_searcher_prof_addr, jit_sandwich_searcher_prof) =
            Self::top_searcher_by_profit(|b| b == MevType::JitSandwich, bundles).unzip();
        let (jit_sandwich_searcher_rev_addr, jit_sandwich_searcher_rev) =
            Self::top_searcher_by_rev(|b| b == MevType::JitSandwich, bundles).unzip();
        let (jit_sandwich_pool_addr, jit_sandwich_pool_prof, jit_sandwich_pool_rev) =
            Self::most_transacted_pool(|b| b == MevType::JitSandwich, bundles, Self::get_pool_fn)
                .three_unzip();
        let (jit_sandwich_pair_addr, jit_sandwich_pair_prof, jit_sandwich_pair_rev) =
            Self::most_transacted_pair(|b| b == MevType::JitSandwich, bundles, Self::get_pair_fn)
                .three_unzip();
        let (jit_sandwich_dex_addr, jit_sandwich_dex_prof, jit_sandwich_dex_rev) =
            Self::most_transacted_dex(|b| b == MevType::JitSandwich, bundles, Self::get_dex_fn)
                .three_unzip();
        // Cex Dex
        let (cex_dex_searcher_prof_addr, cex_dex_searcher_prof) =
            Self::top_searcher_by_profit(|b| b == MevType::CexDex, bundles).unzip();
        let (cex_dex_searcher_rev_addr, cex_dex_searcher_rev) =
            Self::top_searcher_by_rev(|b| b == MevType::CexDex, bundles).unzip();

        let (cex_dex_fund_rev_addr, cex_dex_fund_rev) =
            Self::top_fund_by_type_rev(|b| b == MevType::CexDex, bundles).unzip();
        let (cex_dex_fund_profit_addr, cex_dex_fund_profit) =
            Self::top_fund_by_type_profit(|b| b == MevType::CexDex, bundles).unzip();

        let (cex_dex_pool_addr, cex_dex_pool_prof, cex_dex_pool_rev) =
            Self::most_transacted_pool(|b| b == MevType::CexDex, bundles, Self::get_pool_fn)
                .three_unzip();
        let (cex_dex_pair_addr, cex_dex_pair_prof, cex_dex_pair_rev) =
            Self::most_transacted_pair(|b| b == MevType::CexDex, bundles, Self::get_pair_fn)
                .three_unzip();
        let (cex_dex_dex_addr, cex_dex_dex_prof, cex_dex_dex_rev) =
            Self::most_transacted_dex(|b| b == MevType::CexDex, bundles, Self::get_dex_fn)
                .three_unzip();

        // liquidation
        let (liquidation_searcher_prof_addr, liquidation_searcher_prof) =
            Self::top_searcher_by_profit(|b| b == MevType::Liquidation, bundles).unzip();
        let (liquidation_searcher_rev_addr, liquidation_searcher_rev) =
            Self::top_searcher_by_rev(|b| b == MevType::Liquidation, bundles).unzip();

        let (liq_most_token, liq_most_prof, liq_most_rev) = bundles
            .iter()
            .filter(|b| b.mev_type() == MevType::Liquidation)
            .flat_map(|b| {
                let BundleData::Liquidation(l) = &b.data else { unreachable!() };
                l.liquidations
                    .iter()
                    .map(|l| {
                        (
                            l.collateral_asset.address,
                            (b.header.profit_usd, b.header.profit_usd + b.header.bribe_usd),
                        )
                    })
                    .collect_vec()
            })
            .into_group_map()
            .iter()
            .max_by_key(|v| v.1.len())
            .map(|t| {
                let (p, r): (Vec<_>, Vec<_>) = t.1.iter().copied().unzip();
                (*t.0, p.iter().sum::<f64>(), r.iter().sum::<f64>())
            })
            .three_unzip();

        Self {
            block_number:                    block.block_number,
            all_total_profit:                Self::total_profit_by_type(
                |f| f != MevType::SearcherTx,
                bundles,
            ),
            all_total_revenue:               Self::total_revenue_by_type(
                |f| f != MevType::SearcherTx,
                bundles,
            ),
            all_average_profit_margin:       Self::average_profit_margin(
                |f| f != MevType::SearcherTx,
                bundles,
            )
            .unwrap_or_default(),
            all_searchers:                   Self::unique(|b| b != MevType::SearcherTx, bundles),
            all_top_searcher_rev:            all_rev_am,
            all_top_searcher_rev_addr:       all_rev_addr,
            all_top_searcher_profit_addr:    all_profit_addr,
            all_top_searcher_profit:         all_profit_am,
            all_top_fund_rev_id:             fund_rev,
            all_top_fund_rev:                fund_rev_am,
            all_top_fund_profit:             fund_profit_am,
            all_top_fund_profit_id:          fund_profit,
            all_fund_count:                  Self::unique_funds(
                |b| b != MevType::SearcherTx,
                bundles,
            ),
            all_most_arbed_pool_address:     all_pool,
            all_most_arbed_pool_profit:      all_pool_prof,
            all_most_arbed_pool_revenue:     all_pool_rev,
            all_most_arbed_pair_revenue:     all_pair_rev,
            all_most_arbed_pair_profit:      all_pair_prof,
            all_most_arbed_pair_address:     all_pair,
            // atomic
            atomic_searchers:                Self::unique(|b| b == MevType::AtomicArb, bundles),
            atomic_fund_count:               Self::unique_funds(
                |b| b == MevType::AtomicArb,
                bundles,
            ),
            atomic_total_profit:             Self::total_profit_by_type(
                |b| b == MevType::AtomicArb,
                bundles,
            ),
            atomic_total_revenue:            Self::total_revenue_by_type(
                |b| b == MevType::AtomicArb,
                bundles,
            ),
            atomic_top_searcher_profit_addr: atomic_searcher_prof_addr,
            atomic_top_searcher_rev_addr:    atomic_searcher_rev_addr,
            atomic_top_searcher_profit:      atomic_searcher_prof,
            atomic_top_searcher_rev:         atomic_searcher_rev,
            atomic_top_fund_profit:          atomic_fund_profit,
            atomic_top_fund_profit_id:       atomic_fund_profit_addr,
            atomic_top_fund_rev_id:          atomic_fund_rev_addr,
            atomic_top_fund_rev:             atomic_fund_rev,
            atomic_most_arbed_dex_profit:    atomic_dex_prof,
            atomic_most_arbed_dex_address:   atomic_dex_addr,
            atomic_most_arbed_dex_revenue:   atomic_dex_rev,
            atomic_most_arbed_pair_profit:   atomic_pair_prof,
            atomic_most_arbed_pair_address:  atomic_pair_addr,
            atomic_most_arbed_pair_revenue:  atomic_pair_rev,
            atomic_most_arbed_pool_revenue:  atomic_pool_rev,
            atomic_most_arbed_pool_profit:   atomic_pool_prof,
            atomic_most_arbed_pool_address:  atomic_pool_addr,
            atomic_average_profit_margin:    Self::average_profit_margin(
                |f| f == MevType::AtomicArb,
                bundles,
            )
            .unwrap_or_default(),

            // sandwich
            sandwich_searchers:                Self::unique(|b| b == MevType::Sandwich, bundles),
            sandwich_total_profit:             Self::total_profit_by_type(
                |b| b == MevType::Sandwich,
                bundles,
            ),
            sandwich_total_revenue:            Self::total_revenue_by_type(
                |b| b == MevType::Sandwich,
                bundles,
            ),
            sandwich_biggest_arb_profit:       sandwich_biggest_prof,
            sandwich_biggest_arb_profit_hash:  sandwich_biggest_tx_prof,
            sandwich_biggest_arb_revenue:      sandwich_biggest_rev,
            sandwich_biggest_arb_revenue_hash: sandwich_biggest_tx_rev,
            sandwich_top_searcher_profit_addr: sandwich_searcher_prof_addr,
            sandwich_top_searcher_rev_addr:    sandwich_searcher_rev_addr,
            sandwich_top_searcher_profit:      sandwich_searcher_prof,
            sandwich_top_searcher_rev:         sandwich_searcher_rev,
            sandwich_most_arbed_dex_profit:    sandwich_dex_prof,
            sandwich_most_arbed_dex_address:   sandwich_dex_addr,
            sandwich_most_arbed_dex_revenue:   sandwich_dex_rev,
            sandwich_most_arbed_pair_profit:   sandwich_pair_prof,
            sandwich_most_arbed_pair_address:  sandwich_pair_addr,
            sandwich_most_arbed_pair_revenue:  sandwich_pair_rev,
            sandwich_most_arbed_pool_revenue:  sandwich_pool_rev,
            sandwich_most_arbed_pool_profit:   sandwich_pool_prof,
            sandwich_most_arbed_pool_address:  sandwich_pool_addr,
            sandwich_average_profit_margin:    Self::average_profit_margin(
                |f| f == MevType::Sandwich,
                bundles,
            )
            .unwrap_or_default(),

            // jit
            jit_searchers:                Self::unique(|b| b == MevType::Jit, bundles),
            jit_total_profit:             Self::total_profit_by_type(
                |b| b == MevType::Jit,
                bundles,
            ),
            jit_total_revenue:            Self::total_revenue_by_type(
                |b| b == MevType::Jit,
                bundles,
            ),
            jit_top_searcher_profit_addr: jit_searcher_prof_addr,
            jit_top_searcher_rev_addr:    jit_searcher_rev_addr,
            jit_top_searcher_profit:      jit_searcher_prof,
            jit_top_searcher_rev:         jit_searcher_rev,
            jit_most_arbed_dex_profit:    jit_dex_prof,
            jit_most_arbed_dex_address:   jit_dex_addr,
            jit_most_arbed_dex_revenue:   jit_dex_rev,
            jit_most_arbed_pair_profit:   jit_pair_prof,
            jit_most_arbed_pair_address:  jit_pair_addr,
            jit_most_arbed_pair_revenue:  jit_pair_rev,
            jit_most_arbed_pool_revenue:  jit_pool_rev,
            jit_most_arbed_pool_profit:   jit_pool_prof,
            jit_most_arbed_pool_address:  jit_pool_addr,
            jit_average_profit_margin:    Self::average_profit_margin(
                |f| f == MevType::Jit,
                bundles,
            )
            .unwrap_or_default(),

            // jit sando
            jit_sandwich_searchers:                Self::unique(
                |b| b == MevType::JitSandwich,
                bundles,
            ),
            jit_sandwich_average_profit_margin:    Self::average_profit_margin(
                |f| f == MevType::JitSandwich,
                bundles,
            )
            .unwrap_or_default(),
            jit_sandwich_total_profit:             Self::total_profit_by_type(
                |b| b == MevType::JitSandwich,
                bundles,
            ),
            jit_sandwich_total_revenue:            Self::total_revenue_by_type(
                |b| b == MevType::JitSandwich,
                bundles,
            ),
            jit_sandwich_top_searcher_profit_addr: jit_sandwich_searcher_prof_addr,
            jit_sandwich_top_searcher_rev_addr:    jit_sandwich_searcher_rev_addr,
            jit_sandwich_top_searcher_profit:      jit_sandwich_searcher_prof,
            jit_sandwich_top_searcher_rev:         jit_sandwich_searcher_rev,
            jit_sandwich_most_arbed_dex_profit:    jit_sandwich_dex_prof,
            jit_sandwich_most_arbed_dex_address:   jit_sandwich_dex_addr,
            jit_sandwich_most_arbed_dex_revenue:   jit_sandwich_dex_rev,
            jit_sandwich_most_arbed_pair_profit:   jit_sandwich_pair_prof,
            jit_sandwich_most_arbed_pair_address:  jit_sandwich_pair_addr,
            jit_sandwich_most_arbed_pair_revenue:  jit_sandwich_pair_rev,
            jit_sandwich_most_arbed_pool_revenue:  jit_sandwich_pool_rev,
            jit_sandwich_most_arbed_pool_profit:   jit_sandwich_pool_prof,
            jit_sandwich_most_arbed_pool_address:  jit_sandwich_pool_addr,

            jit_sandwich_biggest_arb_profit:       jit_sandwich_biggest_prof,
            jit_sandwich_biggest_arb_profit_hash:  jit_sandwich_biggest_tx_prof,
            jit_sandwich_biggest_arb_revenue:      jit_sandwich_biggest_rev,
            jit_sandwich_biggest_arb_revenue_hash: jit_sandwich_biggest_tx_rev,

            // cex dex
            cex_dex_searchers:                Self::unique(|b| b == MevType::CexDex, bundles),
            cex_dex_fund_count:               Self::unique_funds(|b| b == MevType::CexDex, bundles),
            cex_dex_total_profit:             Self::total_profit_by_type(
                |f| f == MevType::CexDex,
                bundles,
            ),
            cex_dex_total_revenue:            Self::total_revenue_by_type(
                |f| f == MevType::CexDex,
                bundles,
            ),
            cex_dex_average_profit_margin:    Self::average_profit_margin(
                |f| f == MevType::CexDex,
                bundles,
            )
            .unwrap_or_default(),
            cex_dex_top_searcher_profit_addr: cex_dex_searcher_prof_addr,
            cex_dex_top_searcher_rev_addr:    cex_dex_searcher_rev_addr,
            cex_dex_top_searcher_profit:      cex_dex_searcher_prof,
            cex_dex_top_searcher_rev:         cex_dex_searcher_rev,
            cex_dex_top_fund_profit:          cex_dex_fund_profit,
            cex_dex_top_fund_profit_id:       cex_dex_fund_profit_addr,
            cex_dex_top_fund_rev_id:          cex_dex_fund_rev_addr,
            cex_dex_top_fund_rev:             cex_dex_fund_rev,
            cex_dex_most_arbed_dex_profit:    cex_dex_dex_prof,
            cex_dex_most_arbed_dex_address:   cex_dex_dex_addr,
            cex_dex_most_arbed_dex_revenue:   cex_dex_dex_rev,
            cex_dex_most_arbed_pair_profit:   cex_dex_pair_prof,
            cex_dex_most_arbed_pair_address:  cex_dex_pair_addr,
            cex_dex_most_arbed_pair_revenue:  cex_dex_pair_rev,
            cex_dex_most_arbed_pool_revenue:  cex_dex_pool_rev,
            cex_dex_most_arbed_pool_profit:   cex_dex_pool_prof,
            cex_dex_most_arbed_pool_address:  cex_dex_pool_addr,

            // liquidation
            liquidation_top_searcher_profit_addr: liquidation_searcher_prof_addr,
            liquidation_top_searcher_rev_addr:    liquidation_searcher_rev_addr,
            liquidation_top_searcher_profit:      liquidation_searcher_prof,
            liquidation_top_searcher_rev:         liquidation_searcher_rev,
            liquidation_average_profit_margin:    Self::average_profit_margin(
                |b| b == MevType::Liquidation,
                bundles,
            )
            .unwrap_or_default(),
            liquidation_total_revenue:            Self::total_revenue_by_type(
                |b| b == MevType::Liquidation,
                bundles,
            ),
            liquidation_searchers:                Self::unique(
                |b| b == MevType::Liquidation,
                bundles,
            ),
            liquidation_total_profit:             Self::total_profit_by_type(
                |b| b == MevType::Liquidation,
                bundles,
            ),
            most_liquidated_token_rev:            liq_most_rev,
            most_liquidated_token_profit:         liq_most_prof,
            most_liquidated_token_address:        liq_most_token,
            total_usd_liquidated:                 Self::total_revenue_by_type(
                |b| b == MevType::Liquidation,
                bundles,
            ),
        }
    }

    fn get_pool_fn(data: &BundleData) -> Vec<Address> {
        match data {
            BundleData::Jit(j) => j
                .victim_swaps
                .iter()
                .flatten()
                .map(|s| s.pool)
                .collect::<Vec<_>>(),
            BundleData::JitSandwich(j) => j
                .victim_swaps
                .iter()
                .flatten()
                .map(|s| s.pool)
                .collect::<Vec<_>>(),
            BundleData::CexDex(c) => c.swaps.iter().map(|p| p.pool).collect::<Vec<_>>(),
            BundleData::Sandwich(c) => c
                .victim_swaps
                .iter()
                .flatten()
                .map(|p| p.pool)
                .collect::<Vec<_>>(),
            BundleData::AtomicArb(a) => a.swaps.iter().map(|p| p.pool).collect::<Vec<_>>(),
            _ => vec![],
        }
    }

    fn get_dex_fn(data: &BundleData) -> Vec<Protocol> {
        match data {
            BundleData::Jit(j) => j
                .victim_swaps
                .iter()
                .flatten()
                .map(|s| s.protocol)
                .collect::<Vec<_>>(),
            BundleData::JitSandwich(j) => j
                .victim_swaps
                .iter()
                .flatten()
                .map(|s| s.protocol)
                .collect::<Vec<_>>(),
            BundleData::CexDex(c) => c.swaps.iter().map(|s| s.protocol).collect::<Vec<_>>(),
            BundleData::Sandwich(c) => c
                .victim_swaps
                .iter()
                .flatten()
                .map(|s| s.protocol)
                .collect::<Vec<_>>(),
            BundleData::AtomicArb(a) => a.swaps.iter().map(|s| s.protocol).collect::<Vec<_>>(),
            BundleData::Liquidation(l) => l
                .liquidations
                .iter()
                .map(|l| l.protocol)
                .collect::<Vec<_>>(),
            _ => vec![],
        }
    }

    fn get_pair_fn(data: &BundleData) -> Vec<Pair> {
        match data {
            BundleData::Jit(j) => j
                .victim_swaps
                .iter()
                .flatten()
                .map(|s| Pair(s.token_in.address, s.token_out.address).ordered())
                .collect::<Vec<_>>(),
            BundleData::JitSandwich(j) => j
                .victim_swaps
                .iter()
                .flatten()
                .map(|s| Pair(s.token_in.address, s.token_out.address).ordered())
                .collect::<Vec<_>>(),
            BundleData::CexDex(c) => c
                .swaps
                .iter()
                .map(|s| Pair(s.token_in.address, s.token_out.address).ordered())
                .collect::<Vec<_>>(),
            BundleData::Sandwich(c) => c
                .victim_swaps
                .iter()
                .flatten()
                .map(|s| Pair(s.token_in.address, s.token_out.address).ordered())
                .collect::<Vec<_>>(),
            BundleData::AtomicArb(a) => a
                .swaps
                .iter()
                .map(|s| Pair(s.token_in.address, s.token_out.address).ordered())
                .collect::<Vec<_>>(),
            _ => vec![],
        }
    }

    fn biggest_arb_profit(
        mev_type: impl Fn(MevType) -> bool,
        bundles: &[Bundle],
    ) -> Option<(TxHash, f64)> {
        bundles
            .iter()
            .filter(|b| mev_type(b.data.mev_type()))
            .map(|s| (s.header.tx_hash, s.header.profit_usd))
            .max_by(|a, b| a.1.total_cmp(&b.1))
    }

    fn biggest_arb_revenue(
        mev_type: impl Fn(MevType) -> bool,
        bundles: &[Bundle],
    ) -> Option<(TxHash, f64)> {
        bundles
            .iter()
            .filter(|b| mev_type(b.data.mev_type()))
            .map(|s| (s.header.tx_hash, s.header.profit_usd + s.header.bribe_usd))
            .max_by(|a, b| a.1.total_cmp(&b.1))
    }

    fn total_revenue_by_type(mev_type: impl Fn(MevType) -> bool, bundles: &[Bundle]) -> f64 {
        bundles
            .iter()
            .filter(|b| mev_type(b.data.mev_type()))
            .map(|s| s.header.profit_usd + s.header.bribe_usd)
            .sum::<f64>()
    }

    fn total_profit_by_type(mev_type: impl Fn(MevType) -> bool, bundles: &[Bundle]) -> f64 {
        bundles
            .iter()
            .filter(|b| mev_type(b.data.mev_type()))
            .map(|s| s.header.profit_usd)
            .sum::<f64>()
    }

    fn top_fund_by_type_profit(
        mev_type: impl Fn(MevType) -> bool,
        bundles: &[Bundle],
    ) -> Option<(Fund, f64)> {
        bundles
            .iter()
            .filter(|b| mev_type(b.mev_type()))
            .filter_map(|b| {
                if b.header.fund == Fund::None {
                    None
                } else {
                    Some((b.header.fund, b.header.profit_usd))
                }
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
    }

    fn top_fund_by_type_rev(
        mev_type: impl Fn(MevType) -> bool,
        bundles: &[Bundle],
    ) -> Option<(Fund, f64)> {
        bundles
            .iter()
            .filter(|b| mev_type(b.mev_type()))
            .filter_map(|b| {
                if b.header.fund == Fund::None {
                    None
                } else {
                    Some((b.header.fund, b.header.profit_usd + b.header.bribe_usd))
                }
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
    }

    fn top_searcher_by_profit(
        mev_type: impl Fn(MevType) -> bool,
        bundles: &[Bundle],
    ) -> Option<(Address, f64)> {
        bundles
            .iter()
            .filter(|b| mev_type(b.data.mev_type()))
            .max_by(|a, b| a.header.profit_usd.total_cmp(&b.header.profit_usd))
            .map(|r| (r.header.eoa, r.header.profit_usd))
    }

    fn top_searcher_by_rev(
        mev_type: impl Fn(MevType) -> bool,
        bundles: &[Bundle],
    ) -> Option<(Address, f64)> {
        bundles
            .iter()
            .filter(|b| mev_type(b.data.mev_type()))
            .max_by(|a, b| {
                (a.header.profit_usd + a.header.bribe_usd)
                    .total_cmp(&(b.header.profit_usd + b.header.bribe_usd))
            })
            .map(|r| (r.header.eoa, r.header.profit_usd + r.header.bribe_usd))
    }

    fn most_transacted_pool(
        mev_type: impl Fn(MevType) -> bool,
        bundles: &[Bundle],
        f: impl Fn(&BundleData) -> Vec<Address>,
    ) -> Option<(Address, f64, f64)> {
        Self::most_transacted(mev_type, bundles, f)
    }

    fn most_transacted_pair(
        mev_type: impl Fn(MevType) -> bool,
        bundles: &[Bundle],
        f: impl Fn(&BundleData) -> Vec<Pair>,
    ) -> Option<(Pair, f64, f64)> {
        Self::most_transacted(mev_type, bundles, f)
    }

    fn most_transacted_dex(
        mev_type: impl Fn(MevType) -> bool,
        bundles: &[Bundle],
        f: impl Fn(&BundleData) -> Vec<Protocol>,
    ) -> Option<(Protocol, f64, f64)> {
        Self::most_transacted(mev_type, bundles, f)
    }

    fn average_profit_margin(
        mev_type: impl Fn(MevType) -> bool,
        bundles: &[Bundle],
    ) -> Option<f64> {
        Some(
            bundles
                .iter()
                .filter(|b| mev_type(b.data.mev_type()) && b.header.bribe_usd != 0.0)
                .map(|s| s.header.profit_usd / s.header.bribe_usd)
                .sum::<f64>()
                / Some(
                    bundles
                        .iter()
                        .filter(|b| mev_type(b.data.mev_type()) && b.header.bribe_usd != 0.0)
                        .count(),
                )
                .filter(|value| *value != 0)
                .map(|f| f as f64)?,
        )
    }

    fn unique(mev_type: fn(MevType) -> bool, bundles: &[Bundle]) -> u64 {
        bundles
            .iter()
            .filter(|b| mev_type(b.data.mev_type()))
            .map(|b| b.header.eoa)
            .unique()
            .count() as u64
    }

    fn unique_funds(mev_type: fn(MevType) -> bool, bundles: &[Bundle]) -> u64 {
        bundles
            .iter()
            .filter(|b| mev_type(b.mev_type()))
            .filter_map(|b| if b.header.fund == Fund::None { None } else { Some(b.header.fund) })
            .unique()
            .count() as u64
    }

    fn most_transacted<Ty: Copy + Hash + Eq>(
        mev_type: impl Fn(MevType) -> bool,
        bundles: &[Bundle],
        f: impl Fn(&BundleData) -> Vec<Ty>,
    ) -> Option<(Ty, f64, f64)> {
        bundles
            .iter()
            .filter(|b| mev_type(b.data.mev_type()))
            .flat_map(|b| {
                let res = f(&b.data);
                let mut merged = Vec::with_capacity(res.len());
                for r in res {
                    merged
                        .push((r, (b.header.profit_usd, b.header.profit_usd + b.header.bribe_usd)));
                }
                merged
            })
            .into_group_map()
            .iter()
            .max_by_key(|v| v.1.len())
            .map(|t| {
                let (p, r): (Vec<_>, Vec<_>) = t.1.iter().copied().unzip();
                (*t.0, p.iter().sum::<f64>(), r.iter().sum::<f64>())
            })
    }
}

pub trait ThreeUnzip<A, B, C> {
    fn three_unzip(self) -> (Option<A>, Option<B>, Option<C>)
    where
        Self: Sized;
}

impl<A, B, C> ThreeUnzip<A, B, C> for Option<(A, B, C)> {
    fn three_unzip(self) -> (Option<A>, Option<B>, Option<C>)
    where
        Self: Sized,
    {
        self.map(|i| (Some(i.0), Some(i.1), Some(i.2)))
            .unwrap_or_default()
    }
}
