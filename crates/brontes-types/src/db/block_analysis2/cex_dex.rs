use std::{collections::HashMap, hash::Hash, str::FromStr};

use alloy_primitives::Address;
use clickhouse::Row;
use itertools::Itertools;
use reth_primitives::TxHash;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use super::{utils::*, AnalyzeBlock};
use crate::{
    db::{block_analysis::TokenPairDetails, searcher::Fund, token_info::TokenInfoWithAddress},
    mev::{Bundle, BundleData, Mev, MevBlock, MevType},
    pair::Pair,
    serde_utils::{option_fund, option_protocol, option_txhash, vec_fund, vec_protocol},
    Protocol,
};

#[serde_as]
#[cfg_attr(not(feature = "local-clickhouse"), derive(Serialize, Deserialize))]
#[derive(Debug, Default, Clone)]
pub struct CexDexBlockAnalysis {
    pub cex_dex_total_profit:                f64,
    pub cex_dex_total_revenue:               f64,
    pub cex_dex_average_profit_margin:       f64,
    pub cex_dex_top_searcher_profit:         Option<Address>,
    pub cex_dex_top_searcher_profit_amt:     Option<f64>,
    pub cex_dex_top_searcher_revenue:        Option<Address>,
    pub cex_dex_top_searcher_revenue_amt:    Option<f64>,
    #[serde(rename = "cex_dex_searcher_all.profit")]
    pub cex_dex_searcher_all_profit:         Vec<Address>,
    #[serde(rename = "cex_dex_searcher_all.profit_amt")]
    pub cex_dex_searcher_all_profit_amt:     Vec<f64>,
    #[serde(rename = "cex_dex_searcher_all.revenue")]
    pub cex_dex_searcher_all_revenue:        Vec<Address>,
    #[serde(rename = "cex_dex_searcher_all.revenue_amt")]
    pub cex_dex_searcher_all_revenue_amt:    Vec<f64>,
    pub cex_dex_searcher_count:              u64,
    #[serde(with = "option_fund")]
    pub cex_dex_top_fund_profit:             Option<Fund>,
    pub cex_dex_top_fund_profit_amt:         Option<f64>,
    #[serde(with = "option_fund")]
    pub cex_dex_top_fund_revenue:            Option<Fund>,
    pub cex_dex_top_fund_revenue_amt:        Option<f64>,
    #[serde(rename = "cex_dex_fund_all.profit")]
    #[serde(with = "vec_fund")]
    pub cex_dex_fund_all_profit:             Vec<Fund>,
    #[serde(rename = "cex_dex_fund_all.profit_amt")]
    pub cex_dex_fund_all_profit_amt:         Vec<f64>,
    #[serde(rename = "cex_dex_fund_all.revenue")]
    #[serde(with = "vec_fund")]
    pub cex_dex_fund_all_revenue:            Vec<Fund>,
    #[serde(rename = "cex_dex_fund_all.revenue_amt")]
    pub cex_dex_fund_all_revenue_amt:        Vec<f64>,
    pub cex_dex_fund_count:                  u64,
    pub cex_dex_most_arbed_pool_profit:      Option<Address>,
    pub cex_dex_most_arbed_pool_profit_amt:  Option<f64>,
    pub cex_dex_most_arbed_pool_revenue:     Option<Address>,
    pub cex_dex_most_arbed_pool_revenue_amt: Option<f64>,
    pub cex_dex_most_arbed_pair_profit:      TokenPairDetails,
    pub cex_dex_most_arbed_pair_profit_amt:  Option<f64>,
    pub cex_dex_most_arbed_pair_revenue:     TokenPairDetails,
    pub cex_dex_most_arbed_pair_revenue_amt: Option<f64>,
    #[serde(with = "option_protocol")]
    pub cex_dex_most_arbed_dex_profit:       Option<Protocol>,
    pub cex_dex_most_arbed_dex_profit_amt:   Option<f64>,
    #[serde(with = "option_protocol")]
    pub cex_dex_most_arbed_dex_revenue:      Option<Protocol>,
    pub cex_dex_most_arbed_dex_revenue_amt:  Option<f64>,
    #[serde(rename = "cex_dex_arbed_pool_all.profit")]
    pub cex_dex_arbed_pool_all_profit:       Vec<Address>,
    #[serde(rename = "cex_dex_arbed_pool_all.profit_amt")]
    pub cex_dex_arbed_pool_all_profit_amt:   Vec<f64>,
    #[serde(rename = "cex_dex_arbed_pool_all.revenue")]
    pub cex_dex_arbed_pool_all_revenue:      Vec<Address>,
    #[serde(rename = "cex_dex_arbed_pool_all.revenue_amt")]
    pub cex_dex_arbed_pool_all_revenue_amt:  Vec<f64>,
    #[serde(rename = "cex_dex_arbed_pair_all.profit")]
    pub cex_dex_arbed_pair_all_profit:       Vec<TokenPairDetails>,
    #[serde(rename = "cex_dex_arbed_pair_all.profit_amt")]
    pub cex_dex_arbed_pair_all_profit_amt:   Vec<f64>,
    #[serde(rename = "cex_dex_arbed_pair_all.revenue")]
    pub cex_dex_arbed_pair_all_revenue:      Vec<TokenPairDetails>,
    #[serde(rename = "cex_dex_arbed_pair_all.revenue_amt")]
    pub cex_dex_arbed_pair_all_revenue_amt:  Vec<f64>,
    #[serde(rename = "cex_dex_arbed_dex_all.profit")]
    #[serde(with = "vec_protocol")]
    pub cex_dex_arbed_dex_all_profit:        Vec<Protocol>,
    #[serde(rename = "cex_dex_arbed_dex_all.profit_amt")]
    pub cex_dex_arbed_dex_all_profit_amt:    Vec<f64>,
    #[serde(rename = "cex_dex_arbed_dex_all.revenue")]
    #[serde(with = "vec_protocol")]
    pub cex_dex_arbed_dex_all_revenue:       Vec<Protocol>,
    #[serde(rename = "cex_dex_arbed_dex_all.revenue_amt")]
    pub cex_dex_arbed_dex_all_revenue_amt:   Vec<f64>,
    #[serde(with = "option_txhash")]
    pub cex_dex_biggest_arb_profit:          Option<TxHash>,
    pub cex_dex_biggest_arb_profit_amt:      Option<f64>,
    #[serde(with = "option_txhash")]
    pub cex_dex_biggest_arb_revenue:         Option<TxHash>,
    pub cex_dex_biggest_arb_revenue_amt:     Option<f64>,
}

impl CexDexBlockAnalysis {
    pub fn new(block: &MevBlock, bundles: &[Bundle]) -> Self {
        let (cex_dex_searcher_prof_addr, cex_dex_searcher_prof) =
            Self::top_searcher_by_profit(|b| b == MevType::CexDex, bundles).unzip();
        let (cex_dex_searcher_rev_addr, cex_dex_searcher_rev) =
            Self::top_searcher_by_rev(|b| b == MevType::CexDex, bundles).unzip();

        let (cex_dex_biggest_tx_prof, cex_dex_biggest_prof) =
            Self::biggest_arb_profit(|b| b == MevType::CexDex, bundles).unzip();

        let (cex_dex_biggest_tx_rev, cex_dex_biggest_rev) =
            Self::biggest_arb_revenue(|b| b == MevType::CexDex, bundles).unzip();

        let (cex_dex_all_searcher_prof_addr, cex_dex_all_searcher_prof) =
            Self::all_searchers_by_profit(|b| b == MevType::CexDex, bundles)
                .into_iter()
                .unzip();

        let (cex_dex_all_funds_rev_addr, cex_dex_all_funds_rev) =
            Self::all_funds_by_type_rev(|b| b == MevType::CexDex, bundles)
                .into_iter()
                .unzip();
        let (cex_dex_all_funds_profit_addr, cex_dex_all_funds_profit) =
            Self::all_funds_by_type_profit(|b| b == MevType::CexDex, bundles)
                .into_iter()
                .unzip();

        let (cex_dex_all_searcher_rev_addr, cex_dex_all_searcher_rev) =
            Self::all_searchers_by_rev(|b| b == MevType::CexDex, bundles)
                .into_iter()
                .unzip();

        let (cex_dex_fund_rev_addr, cex_dex_fund_rev) =
            Self::top_fund_by_type_rev(|b| b == MevType::CexDex, bundles).unzip();
        let (cex_dex_fund_profit_addr, cex_dex_fund_profit) =
            Self::top_fund_by_type_profit(|b| b == MevType::CexDex, bundles).unzip();

        let (cex_dex_pool_addr_prof, cex_dex_pool_addr_rev, cex_dex_pool_prof, cex_dex_pool_rev) =
            Self::most_transacted_pool(|b| b == MevType::CexDex, bundles, Self::get_pool_fn)
                .four_unzip();
        let (cex_dex_pair_addr_prof, cex_dex_pair_addr_rev, cex_dex_pair_prof, cex_dex_pair_rev) =
            Self::most_transacted_pair(|b| b == MevType::CexDex, bundles, Self::get_pair_fn)
                .unwrap_or_default();
        let (cex_dex_dex_addr_prof, cex_dex_dex_addr_rev, cex_dex_dex_prof, cex_dex_dex_rev) =
            Self::most_transacted_dex(|b| b == MevType::CexDex, bundles, Self::get_dex_fn)
                .four_unzip();

        let (
            cex_dex_all_pools_addr_prof,
            cex_dex_all_pools_prof,
            cex_dex_all_pools_addr_rev,
            cex_dex_all_pools_rev,
        ) = Self::all_transacted_pools(|b| b == MevType::CexDex, bundles, Self::get_pool_fn)
            .four_unzip();

        let (
            cex_dex_all_pairs_addr_prof,
            cex_dex_all_pairs_prof,
            cex_dex_all_pairs_addr_rev,
            cex_dex_all_pairs_rev,
        ) = Self::all_transacted_pairs(|b| b == MevType::CexDex, bundles, Self::get_pair_fn)
            .four_unzip();

        let (
            cex_dex_all_dexes_addr_prof,
            cex_dex_all_dexes_prof,
            cex_dex_all_dexes_addr_rev,
            cex_dex_all_dexes_rev,
        ) = Self::all_transacted_dexes(|b| b == MevType::CexDex, bundles, Self::get_dex_fn)
            .four_unzip();

        Self {
            cex_dex_searcher_count:              Self::unique(|b| b == MevType::CexDex, bundles),
            cex_dex_fund_count:                  Self::unique_funds(
                |b| b == MevType::CexDex,
                bundles,
            ),
            cex_dex_total_profit:                Self::total_profit_by_type(
                |f| f == MevType::CexDex,
                bundles,
            ),
            cex_dex_total_revenue:               Self::total_revenue_by_type(
                |f| f == MevType::CexDex,
                bundles,
            ),
            cex_dex_average_profit_margin:       Self::average_profit_margin(
                |f| f == MevType::CexDex,
                bundles,
            )
            .unwrap_or_default(),
            cex_dex_top_searcher_profit:         cex_dex_searcher_prof_addr,
            cex_dex_top_searcher_revenue:        cex_dex_searcher_rev_addr,
            cex_dex_top_searcher_profit_amt:     cex_dex_searcher_prof,
            cex_dex_top_searcher_revenue_amt:    cex_dex_searcher_rev,
            cex_dex_top_fund_profit_amt:         cex_dex_fund_profit,
            cex_dex_top_fund_profit:             cex_dex_fund_profit_addr,
            cex_dex_top_fund_revenue:            cex_dex_fund_rev_addr,
            cex_dex_top_fund_revenue_amt:        cex_dex_fund_rev,
            cex_dex_most_arbed_dex_profit_amt:   cex_dex_dex_prof,
            cex_dex_most_arbed_dex_profit:       cex_dex_dex_addr_prof,
            cex_dex_most_arbed_dex_revenue:      cex_dex_dex_addr_rev,
            cex_dex_most_arbed_dex_revenue_amt:  cex_dex_dex_rev,
            cex_dex_most_arbed_pair_profit_amt:  Some(cex_dex_pair_prof),
            cex_dex_most_arbed_pair_profit:      cex_dex_pair_addr_prof,
            cex_dex_most_arbed_pair_revenue:     cex_dex_pair_addr_rev,
            cex_dex_most_arbed_pair_revenue_amt: Some(cex_dex_pair_rev),
            cex_dex_most_arbed_pool_revenue_amt: cex_dex_pool_rev,
            cex_dex_most_arbed_pool_profit_amt:  cex_dex_pool_prof,
            cex_dex_most_arbed_pool_profit:      cex_dex_pool_addr_prof,
            cex_dex_most_arbed_pool_revenue:     cex_dex_pool_addr_rev,
            cex_dex_biggest_arb_profit:          cex_dex_biggest_tx_prof,
            cex_dex_biggest_arb_profit_amt:      cex_dex_biggest_prof,
            cex_dex_biggest_arb_revenue:         cex_dex_biggest_tx_rev,
            cex_dex_biggest_arb_revenue_amt:     cex_dex_biggest_rev,
            cex_dex_searcher_all_profit:         cex_dex_all_searcher_prof_addr,
            cex_dex_searcher_all_profit_amt:     cex_dex_all_searcher_prof,
            cex_dex_searcher_all_revenue:        cex_dex_all_searcher_rev_addr,
            cex_dex_searcher_all_revenue_amt:    cex_dex_all_searcher_rev,
            cex_dex_fund_all_profit:             cex_dex_all_funds_profit_addr,
            cex_dex_fund_all_profit_amt:         cex_dex_all_funds_profit,
            cex_dex_fund_all_revenue:            cex_dex_all_funds_rev_addr,
            cex_dex_fund_all_revenue_amt:        cex_dex_all_funds_rev,
            cex_dex_arbed_dex_all_profit:        cex_dex_all_dexes_addr_prof,
            cex_dex_arbed_dex_all_profit_amt:    cex_dex_all_dexes_prof,
            cex_dex_arbed_dex_all_revenue:       cex_dex_all_dexes_addr_rev,
            cex_dex_arbed_dex_all_revenue_amt:   cex_dex_all_dexes_rev,
            cex_dex_arbed_pair_all_profit:       cex_dex_all_pairs_addr_prof,
            cex_dex_arbed_pair_all_profit_amt:   cex_dex_all_pairs_prof,
            cex_dex_arbed_pair_all_revenue:      cex_dex_all_pairs_addr_rev,
            cex_dex_arbed_pair_all_revenue_amt:  cex_dex_all_pairs_rev,
            cex_dex_arbed_pool_all_profit:       cex_dex_all_pools_addr_prof,
            cex_dex_arbed_pool_all_profit_amt:   cex_dex_all_pools_prof,
            cex_dex_arbed_pool_all_revenue:      cex_dex_all_pools_addr_rev,
            cex_dex_arbed_pool_all_revenue_amt:  cex_dex_all_pools_rev,
        }
    }
}

impl AnalyzeBlock for CexDexBlockAnalysis {}

#[cfg(feature = "local-clickhouse")]
impl Serialize for CexDexBlockAnalysis {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
    }
}

// impl<'de> Deserialize<'de> for CexDexBlockAnalysis {

// }

#[derive(Serialize)]
struct AAA {
    s: CexDexBlockAnalysis,
}
