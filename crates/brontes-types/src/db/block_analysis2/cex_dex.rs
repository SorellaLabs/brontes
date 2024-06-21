use std::{collections::HashMap, hash::Hash, str::FromStr};

use alloy_primitives::Address;
use clickhouse::Row;
use itertools::Itertools;
use reth_primitives::TxHash;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::{
    db::{block_analysis::TokenPairDetails, searcher::Fund, token_info::TokenInfoWithAddress},
    mev::{Bundle, BundleData, Mev, MevBlock, MevType},
    pair::Pair,
    serde_utils::{option_fund, option_protocol, option_txhash, vec_fund, vec_protocol},
    Protocol,
};

#[serde_as]
#[derive(Debug, Default, Clone, Serialize, Deserialize, Row)]
struct CexDexBlockAnalysis {
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
    #[serde(rename = "cex_dex_arbed_pair_all.profit")]
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
