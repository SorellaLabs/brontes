use crate::serde_utils::address;
use crate::serde_utils::vec_address;
use crate::{
    db::{searcher::Fund, token_info::TokenInfoWithAddress},
    mev::{Bundle, BundleData, Mev, MevBlock, MevType},
    pair::Pair,
    serde_utils::{
        option_address, option_fund, option_protocol, option_txhash, vec_fund, vec_protocol,
    },
    Protocol,
};
use alloy_primitives::Address;
use alloy_primitives::TxHash;
use clickhouse::Row;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::{collections::HashMap, hash::Hash, str::FromStr};

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, Row)]
pub struct BlockAnalysis {
    pub block_number: u64,
    pub eth_price: f64,
    // all
    pub all_total_profit: f64,
    pub all_total_revenue: f64,
    pub all_bundle_count: u64,
    pub all_average_profit_margin: f64,
    #[serde(with = "option_address")]
    pub all_top_searcher_profit: Option<Address>,
    pub all_top_searcher_profit_amt: Option<f64>,
    #[serde(with = "option_address")]
    pub all_top_searcher_revenue: Option<Address>,
    pub all_top_searcher_revenue_amt: Option<f64>,
    pub all_searcher_count: u64,
    #[serde(with = "option_fund")]
    pub all_top_fund_profit: Option<Fund>,
    pub all_top_fund_profit_amt: Option<f64>,
    #[serde(with = "option_fund")]
    pub all_top_fund_revenue: Option<Fund>,
    pub all_top_fund_revenue_amt: Option<f64>,
    pub all_fund_count: u64,
    #[serde(with = "option_address")]
    pub all_most_arbed_pool_profit: Option<Address>,
    pub all_most_arbed_pool_profit_amt: Option<f64>,
    #[serde(with = "option_address")]
    pub all_most_arbed_pool_revenue: Option<Address>,
    pub all_most_arbed_pool_revenue_amt: Option<f64>,
    pub all_most_arbed_pair_profit: TokenPairDetails,
    pub all_most_arbed_pair_profit_amt: Option<f64>,
    pub all_most_arbed_pair_revenue: TokenPairDetails,
    pub all_most_arbed_pair_revenue_amt: Option<f64>,
    #[serde(with = "option_protocol")]
    pub all_most_arbed_dex_profit: Option<Protocol>,
    pub all_most_arbed_dex_profit_amt: Option<f64>,
    #[serde(with = "option_protocol")]
    pub all_most_arbed_dex_revenue: Option<Protocol>,
    pub all_most_arbed_dex_revenue_amt: Option<f64>,
    #[serde(with = "option_txhash")]
    pub all_biggest_arb_profit: Option<TxHash>,
    pub all_biggest_arb_profit_amt: Option<f64>,
    #[serde(with = "option_txhash")]
    pub all_biggest_arb_revenue: Option<TxHash>,
    pub all_biggest_arb_revenue_amt: Option<f64>,

    // atomic
    pub atomic_bundle_count: u64,
    pub atomic_total_profit: f64,
    pub atomic_total_revenue: f64,
    pub atomic_average_profit_margin: f64,
    #[serde(with = "option_address")]
    pub atomic_top_searcher_profit: Option<Address>,
    pub atomic_top_searcher_profit_amt: Option<f64>,
    #[serde(with = "option_address")]
    pub atomic_top_searcher_revenue: Option<Address>,
    pub atomic_top_searcher_revenue_amt: Option<f64>,
    #[serde(with = "vec_address")]
    #[serde(rename = "atomic_searcher_eoa_all.profit")]
    pub atomic_searcher_eoa_all_profit: Vec<Address>,
    #[serde(rename = "atomic_searcher_eoa_all.profit_amt")]
    pub atomic_searcher_eoa_all_profit_amt: Vec<f64>,
    #[serde(with = "vec_address")]
    #[serde(rename = "atomic_searcher_eoa_all.revenue")]
    pub atomic_searcher_eoa_all_revenue: Vec<Address>,
    #[serde(rename = "atomic_searcher_eoa_all.revenue_amt")]
    pub atomic_searcher_eoa_all_revenue_amt: Vec<f64>,
    pub atomic_searcher_eoa_count: u64,
    #[serde(with = "vec_address")]
    #[serde(rename = "atomic_mev_contract_all.profit")]
    pub atomic_mev_contract_all_profit: Vec<Address>,
    #[serde(rename = "atomic_mev_contract_all.profit_amt")]
    pub atomic_mev_contract_all_profit_amt: Vec<f64>,
    #[serde(with = "vec_address")]
    #[serde(rename = "atomic_mev_contract_all.revenue")]
    pub atomic_mev_contract_all_revenue: Vec<Address>,
    #[serde(rename = "atomic_mev_contract_all.revenue_amt")]
    pub atomic_mev_contract_all_revenue_amt: Vec<f64>,
    pub atomic_mev_contract_count: u64,
    #[serde(with = "option_fund")]
    pub atomic_top_fund_profit: Option<Fund>,
    pub atomic_top_fund_profit_amt: Option<f64>,
    #[serde(with = "option_fund")]
    pub atomic_top_fund_revenue: Option<Fund>,
    pub atomic_top_fund_revenue_amt: Option<f64>,
    #[serde(rename = "atomic_fund_all.profit")]
    #[serde(with = "vec_fund")]
    pub atomic_fund_all_profit: Vec<Fund>,
    #[serde(rename = "atomic_fund_all.profit_amt")]
    pub atomic_fund_all_profit_amt: Vec<f64>,
    #[serde(rename = "atomic_fund_all.revenue")]
    #[serde(with = "vec_fund")]
    pub atomic_fund_all_revenue: Vec<Fund>,
    #[serde(rename = "atomic_fund_all.revenue_amt")]
    pub atomic_fund_all_revenue_amt: Vec<f64>,
    pub atomic_fund_count: u64,
    #[serde(with = "option_address")]
    pub atomic_most_arbed_pool_profit: Option<Address>,
    pub atomic_most_arbed_pool_profit_amt: Option<f64>,
    #[serde(with = "option_address")]
    pub atomic_most_arbed_pool_revenue: Option<Address>,
    pub atomic_most_arbed_pool_revenue_amt: Option<f64>,
    pub atomic_most_arbed_pair_profit: TokenPairDetails,
    pub atomic_most_arbed_pair_profit_amt: Option<f64>,
    pub atomic_most_arbed_pair_revenue: TokenPairDetails,
    pub atomic_most_arbed_pair_revenue_amt: Option<f64>,
    #[serde(with = "option_protocol")]
    pub atomic_most_arbed_dex_profit: Option<Protocol>,
    pub atomic_most_arbed_dex_profit_amt: Option<f64>,
    #[serde(with = "option_protocol")]
    pub atomic_most_arbed_dex_revenue: Option<Protocol>,
    pub atomic_most_arbed_dex_revenue_amt: Option<f64>,
    #[serde(with = "vec_address")]
    #[serde(rename = "atomic_arbed_pool_all.profit")]
    pub atomic_arbed_pool_all_profit: Vec<Address>,
    #[serde(rename = "atomic_arbed_pool_all.profit_amt")]
    pub atomic_arbed_pool_all_profit_amt: Vec<f64>,
    #[serde(with = "vec_address")]
    #[serde(rename = "atomic_arbed_pool_all.revenue")]
    pub atomic_arbed_pool_all_revenue: Vec<Address>,
    #[serde(rename = "atomic_arbed_pool_all.revenue_amt")]
    pub atomic_arbed_pool_all_revenue_amt: Vec<f64>,
    #[serde(rename = "atomic_arbed_pair_all.profit")]
    pub atomic_arbed_pair_all_profit: Vec<TokenPairDetails>,
    #[serde(rename = "atomic_arbed_pair_all.profit_amt")]
    pub atomic_arbed_pair_all_profit_amt: Vec<f64>,
    #[serde(rename = "atomic_arbed_pair_all.revenue")]
    pub atomic_arbed_pair_all_revenue: Vec<TokenPairDetails>,
    #[serde(rename = "atomic_arbed_pair_all.revenue_amt")]
    pub atomic_arbed_pair_all_revenue_amt: Vec<f64>,
    #[serde(rename = "atomic_arbed_dex_all.profit")]
    #[serde(with = "vec_protocol")]
    pub atomic_arbed_dex_all_profit: Vec<Protocol>,
    #[serde(rename = "atomic_arbed_dex_all.profit_amt")]
    pub atomic_arbed_dex_all_profit_amt: Vec<f64>,
    #[serde(rename = "atomic_arbed_dex_all.revenue")]
    #[serde(with = "vec_protocol")]
    pub atomic_arbed_dex_all_revenue: Vec<Protocol>,
    #[serde(rename = "atomic_arbed_dex_all.revenue_amt")]
    pub atomic_arbed_dex_all_revenue_amt: Vec<f64>,
    #[serde(with = "option_txhash")]
    pub atomic_biggest_arb_profit: Option<TxHash>,
    pub atomic_biggest_arb_profit_amt: Option<f64>,
    #[serde(with = "option_txhash")]
    pub atomic_biggest_arb_revenue: Option<TxHash>,
    pub atomic_biggest_arb_revenue_amt: Option<f64>,

    // sandwich
    pub sandwich_bundle_count: u64,
    pub sandwich_total_profit: f64,
    pub sandwich_total_revenue: f64,
    pub sandwich_average_profit_margin: f64,
    #[serde(with = "option_address")]
    pub sandwich_top_searcher_profit: Option<Address>,
    pub sandwich_top_searcher_profit_amt: Option<f64>,
    #[serde(with = "option_address")]
    pub sandwich_top_searcher_revenue: Option<Address>,
    pub sandwich_top_searcher_revenue_amt: Option<f64>,
    #[serde(with = "vec_address")]
    #[serde(rename = "sandwich_searcher_eoa_all.profit")]
    pub sandwich_searcher_eoa_all_profit: Vec<Address>,
    #[serde(rename = "sandwich_searcher_eoa_all.profit_amt")]
    pub sandwich_searcher_eoa_all_profit_amt: Vec<f64>,
    #[serde(with = "vec_address")]
    #[serde(rename = "sandwich_searcher_eoa_all.revenue")]
    pub sandwich_searcher_eoa_all_revenue: Vec<Address>,
    #[serde(rename = "sandwich_searcher_eoa_all.revenue_amt")]
    pub sandwich_searcher_eoa_all_revenue_amt: Vec<f64>,
    pub sandwich_searcher_eoa_count: u64,
    #[serde(with = "vec_address")]
    #[serde(rename = "sandwich_mev_contract_all.profit")]
    pub sandwich_mev_contract_all_profit: Vec<Address>,
    #[serde(rename = "sandwich_mev_contract_all.profit_amt")]
    pub sandwich_mev_contract_all_profit_amt: Vec<f64>,
    #[serde(with = "vec_address")]
    #[serde(rename = "sandwich_mev_contract_all.revenue")]
    pub sandwich_mev_contract_all_revenue: Vec<Address>,
    #[serde(rename = "sandwich_mev_contract_all.revenue_amt")]
    pub sandwich_mev_contract_all_revenue_amt: Vec<f64>,
    pub sandwich_mev_contract_count: u64,
    #[serde(with = "option_fund")]
    pub sandwich_top_fund_profit: Option<Fund>,
    pub sandwich_top_fund_profit_amt: Option<f64>,
    #[serde(with = "option_fund")]
    pub sandwich_top_fund_revenue: Option<Fund>,
    pub sandwich_top_fund_revenue_amt: Option<f64>,
    #[serde(rename = "sandwich_fund_all.profit")]
    #[serde(with = "vec_fund")]
    pub sandwich_fund_all_profit: Vec<Fund>,
    #[serde(rename = "sandwich_fund_all.profit_amt")]
    pub sandwich_fund_all_profit_amt: Vec<f64>,
    #[serde(rename = "sandwich_fund_all.revenue")]
    #[serde(with = "vec_fund")]
    pub sandwich_fund_all_revenue: Vec<Fund>,
    #[serde(rename = "sandwich_fund_all.revenue_amt")]
    pub sandwich_fund_all_revenue_amt: Vec<f64>,
    pub sandwich_fund_count: u64,
    #[serde(with = "option_address")]
    pub sandwich_most_arbed_pool_profit: Option<Address>,
    pub sandwich_most_arbed_pool_profit_amt: Option<f64>,
    #[serde(with = "option_address")]
    pub sandwich_most_arbed_pool_revenue: Option<Address>,
    pub sandwich_most_arbed_pool_revenue_amt: Option<f64>,
    pub sandwich_most_arbed_pair_profit: TokenPairDetails,
    pub sandwich_most_arbed_pair_profit_amt: Option<f64>,
    pub sandwich_most_arbed_pair_revenue: TokenPairDetails,
    pub sandwich_most_arbed_pair_revenue_amt: Option<f64>,
    #[serde(with = "option_protocol")]
    pub sandwich_most_arbed_dex_profit: Option<Protocol>,
    pub sandwich_most_arbed_dex_profit_amt: Option<f64>,
    #[serde(with = "option_protocol")]
    pub sandwich_most_arbed_dex_revenue: Option<Protocol>,
    pub sandwich_most_arbed_dex_revenue_amt: Option<f64>,
    #[serde(with = "vec_address")]
    #[serde(rename = "sandwich_arbed_pool_all.profit")]
    pub sandwich_arbed_pool_all_profit: Vec<Address>,
    #[serde(rename = "sandwich_arbed_pool_all.profit_amt")]
    pub sandwich_arbed_pool_all_profit_amt: Vec<f64>,
    #[serde(with = "vec_address")]
    #[serde(rename = "sandwich_arbed_pool_all.revenue")]
    pub sandwich_arbed_pool_all_revenue: Vec<Address>,
    #[serde(rename = "sandwich_arbed_pool_all.revenue_amt")]
    pub sandwich_arbed_pool_all_revenue_amt: Vec<f64>,
    #[serde(rename = "sandwich_arbed_pair_all.profit")]
    pub sandwich_arbed_pair_all_profit: Vec<TokenPairDetails>,
    #[serde(rename = "sandwich_arbed_pair_all.profit_amt")]
    pub sandwich_arbed_pair_all_profit_amt: Vec<f64>,
    #[serde(rename = "sandwich_arbed_pair_all.revenue")]
    pub sandwich_arbed_pair_all_revenue: Vec<TokenPairDetails>,
    #[serde(rename = "sandwich_arbed_pair_all.revenue_amt")]
    pub sandwich_arbed_pair_all_revenue_amt: Vec<f64>,
    #[serde(rename = "sandwich_arbed_dex_all.profit")]
    #[serde(with = "vec_protocol")]
    pub sandwich_arbed_dex_all_profit: Vec<Protocol>,
    #[serde(rename = "sandwich_arbed_dex_all.profit_amt")]
    pub sandwich_arbed_dex_all_profit_amt: Vec<f64>,
    #[serde(rename = "sandwich_arbed_dex_all.revenue")]
    #[serde(with = "vec_protocol")]
    pub sandwich_arbed_dex_all_revenue: Vec<Protocol>,
    #[serde(rename = "sandwich_arbed_dex_all.revenue_amt")]
    pub sandwich_arbed_dex_all_revenue_amt: Vec<f64>,
    #[serde(with = "option_txhash")]
    pub sandwich_biggest_arb_profit: Option<TxHash>,
    pub sandwich_biggest_arb_profit_amt: Option<f64>,
    #[serde(with = "option_txhash")]
    pub sandwich_biggest_arb_revenue: Option<TxHash>,
    pub sandwich_biggest_arb_revenue_amt: Option<f64>,

    // jit
    pub jit_bundle_count: u64,
    pub jit_total_profit: f64,
    pub jit_total_revenue: f64,
    pub jit_average_profit_margin: f64,
    #[serde(with = "option_address")]
    pub jit_top_searcher_profit: Option<Address>,
    pub jit_top_searcher_profit_amt: Option<f64>,
    #[serde(with = "option_address")]
    pub jit_top_searcher_revenue: Option<Address>,
    pub jit_top_searcher_revenue_amt: Option<f64>,
    #[serde(with = "vec_address")]
    #[serde(rename = "jit_searcher_eoa_all.profit")]
    pub jit_searcher_eoa_all_profit: Vec<Address>,
    #[serde(rename = "jit_searcher_eoa_all.profit_amt")]
    pub jit_searcher_eoa_all_profit_amt: Vec<f64>,
    #[serde(with = "vec_address")]
    #[serde(rename = "jit_searcher_eoa_all.revenue")]
    pub jit_searcher_eoa_all_revenue: Vec<Address>,
    #[serde(rename = "jit_searcher_eoa_all.revenue_amt")]
    pub jit_searcher_eoa_all_revenue_amt: Vec<f64>,
    pub jit_searcher_eoa_count: u64,
    #[serde(with = "vec_address")]
    #[serde(rename = "jit_mev_contract_all.profit")]
    pub jit_mev_contract_all_profit: Vec<Address>,
    #[serde(rename = "jit_mev_contract_all.profit_amt")]
    pub jit_mev_contract_all_profit_amt: Vec<f64>,
    #[serde(with = "vec_address")]
    #[serde(rename = "jit_mev_contract_all.revenue")]
    pub jit_mev_contract_all_revenue: Vec<Address>,
    #[serde(rename = "jit_mev_contract_all.revenue_amt")]
    pub jit_mev_contract_all_revenue_amt: Vec<f64>,
    pub jit_mev_contract_count: u64,
    #[serde(with = "option_fund")]
    pub jit_top_fund_profit: Option<Fund>,
    pub jit_top_fund_profit_amt: Option<f64>,
    #[serde(with = "option_fund")]
    pub jit_top_fund_revenue: Option<Fund>,
    pub jit_top_fund_revenue_amt: Option<f64>,
    #[serde(rename = "jit_fund_all.profit")]
    #[serde(with = "vec_fund")]
    pub jit_fund_all_profit: Vec<Fund>,
    #[serde(rename = "jit_fund_all.profit_amt")]
    pub jit_fund_all_profit_amt: Vec<f64>,
    #[serde(rename = "jit_fund_all.revenue")]
    #[serde(with = "vec_fund")]
    pub jit_fund_all_revenue: Vec<Fund>,
    #[serde(rename = "jit_fund_all.revenue_amt")]
    pub jit_fund_all_revenue_amt: Vec<f64>,
    pub jit_fund_count: u64,
    #[serde(with = "option_address")]
    pub jit_most_arbed_pool_profit: Option<Address>,
    pub jit_most_arbed_pool_profit_amt: Option<f64>,
    #[serde(with = "option_address")]
    pub jit_most_arbed_pool_revenue: Option<Address>,
    pub jit_most_arbed_pool_revenue_amt: Option<f64>,
    pub jit_most_arbed_pair_profit: TokenPairDetails,
    pub jit_most_arbed_pair_profit_amt: Option<f64>,
    pub jit_most_arbed_pair_revenue: TokenPairDetails,
    pub jit_most_arbed_pair_revenue_amt: Option<f64>,
    #[serde(with = "option_protocol")]
    pub jit_most_arbed_dex_profit: Option<Protocol>,
    pub jit_most_arbed_dex_profit_amt: Option<f64>,
    #[serde(with = "option_protocol")]
    pub jit_most_arbed_dex_revenue: Option<Protocol>,
    pub jit_most_arbed_dex_revenue_amt: Option<f64>,
    #[serde(with = "vec_address")]
    #[serde(rename = "jit_arbed_pool_all.profit")]
    pub jit_arbed_pool_all_profit: Vec<Address>,
    #[serde(rename = "jit_arbed_pool_all.profit_amt")]
    pub jit_arbed_pool_all_profit_amt: Vec<f64>,
    #[serde(with = "vec_address")]
    #[serde(rename = "jit_arbed_pool_all.revenue")]
    pub jit_arbed_pool_all_revenue: Vec<Address>,
    #[serde(rename = "jit_arbed_pool_all.revenue_amt")]
    pub jit_arbed_pool_all_revenue_amt: Vec<f64>,
    #[serde(rename = "jit_arbed_pair_all.profit")]
    pub jit_arbed_pair_all_profit: Vec<TokenPairDetails>,
    #[serde(rename = "jit_arbed_pair_all.profit_amt")]
    pub jit_arbed_pair_all_profit_amt: Vec<f64>,
    #[serde(rename = "jit_arbed_pair_all.revenue")]
    pub jit_arbed_pair_all_revenue: Vec<TokenPairDetails>,
    #[serde(rename = "jit_arbed_pair_all.revenue_amt")]
    pub jit_arbed_pair_all_revenue_amt: Vec<f64>,
    #[serde(rename = "jit_arbed_dex_all.profit")]
    #[serde(with = "vec_protocol")]
    pub jit_arbed_dex_all_profit: Vec<Protocol>,
    #[serde(rename = "jit_arbed_dex_all.profit_amt")]
    pub jit_arbed_dex_all_profit_amt: Vec<f64>,
    #[serde(rename = "jit_arbed_dex_all.revenue")]
    #[serde(with = "vec_protocol")]
    pub jit_arbed_dex_all_revenue: Vec<Protocol>,
    #[serde(rename = "jit_arbed_dex_all.revenue_amt")]
    pub jit_arbed_dex_all_revenue_amt: Vec<f64>,
    #[serde(with = "option_txhash")]
    pub jit_biggest_arb_profit: Option<TxHash>,
    pub jit_biggest_arb_profit_amt: Option<f64>,
    #[serde(with = "option_txhash")]
    pub jit_biggest_arb_revenue: Option<TxHash>,
    pub jit_biggest_arb_revenue_amt: Option<f64>,

    // jit-sandwich
    pub jit_sandwich_bundle_count: u64,
    pub jit_sandwich_total_profit: f64,
    pub jit_sandwich_total_revenue: f64,
    pub jit_sandwich_average_profit_margin: f64,
    #[serde(with = "option_address")]
    pub jit_sandwich_top_searcher_profit: Option<Address>,
    pub jit_sandwich_top_searcher_profit_amt: Option<f64>,
    #[serde(with = "option_address")]
    pub jit_sandwich_top_searcher_revenue: Option<Address>,
    pub jit_sandwich_top_searcher_revenue_amt: Option<f64>,
    #[serde(with = "vec_address")]
    #[serde(rename = "jit_sandwich_searcher_eoa_all.profit")]
    pub jit_sandwich_searcher_eoa_all_profit: Vec<Address>,
    #[serde(rename = "jit_sandwich_searcher_eoa_all.profit_amt")]
    pub jit_sandwich_searcher_eoa_all_profit_amt: Vec<f64>,
    #[serde(with = "vec_address")]
    #[serde(rename = "jit_sandwich_searcher_eoa_all.revenue")]
    pub jit_sandwich_searcher_eoa_all_revenue: Vec<Address>,
    #[serde(rename = "jit_sandwich_searcher_eoa_all.revenue_amt")]
    pub jit_sandwich_searcher_eoa_all_revenue_amt: Vec<f64>,
    pub jit_sandwich_searcher_eoa_count: u64,
    #[serde(with = "vec_address")]
    #[serde(rename = "jit_sandwich_mev_contract_all.profit")]
    pub jit_sandwich_mev_contract_all_profit: Vec<Address>,
    #[serde(rename = "jit_sandwich_mev_contract_all.profit_amt")]
    pub jit_sandwich_mev_contract_all_profit_amt: Vec<f64>,
    #[serde(with = "vec_address")]
    #[serde(rename = "jit_sandwich_mev_contract_all.revenue")]
    pub jit_sandwich_mev_contract_all_revenue: Vec<Address>,
    #[serde(rename = "jit_sandwich_mev_contract_all.revenue_amt")]
    pub jit_sandwich_mev_contract_all_revenue_amt: Vec<f64>,
    pub jit_sandwich_mev_contract_count: u64,
    #[serde(with = "option_fund")]
    pub jit_sandwich_top_fund_profit: Option<Fund>,
    pub jit_sandwich_top_fund_profit_amt: Option<f64>,
    #[serde(with = "option_fund")]
    pub jit_sandwich_top_fund_revenue: Option<Fund>,
    pub jit_sandwich_top_fund_revenue_amt: Option<f64>,
    #[serde(rename = "jit_sandwich_fund_all.profit")]
    #[serde(with = "vec_fund")]
    pub jit_sandwich_fund_all_profit: Vec<Fund>,
    #[serde(rename = "jit_sandwich_fund_all.profit_amt")]
    pub jit_sandwich_fund_all_profit_amt: Vec<f64>,
    #[serde(rename = "jit_sandwich_fund_all.revenue")]
    #[serde(with = "vec_fund")]
    pub jit_sandwich_fund_all_revenue: Vec<Fund>,
    #[serde(rename = "jit_sandwich_fund_all.revenue_amt")]
    pub jit_sandwich_fund_all_revenue_amt: Vec<f64>,
    pub jit_sandwich_fund_count: u64,
    #[serde(with = "option_address")]
    pub jit_sandwich_most_arbed_pool_profit: Option<Address>,
    pub jit_sandwich_most_arbed_pool_profit_amt: Option<f64>,
    #[serde(with = "option_address")]
    pub jit_sandwich_most_arbed_pool_revenue: Option<Address>,
    pub jit_sandwich_most_arbed_pool_revenue_amt: Option<f64>,
    pub jit_sandwich_most_arbed_pair_profit: TokenPairDetails,
    pub jit_sandwich_most_arbed_pair_profit_amt: Option<f64>,
    pub jit_sandwich_most_arbed_pair_revenue: TokenPairDetails,
    pub jit_sandwich_most_arbed_pair_revenue_amt: Option<f64>,
    #[serde(with = "option_protocol")]
    pub jit_sandwich_most_arbed_dex_profit: Option<Protocol>,
    pub jit_sandwich_most_arbed_dex_profit_amt: Option<f64>,
    #[serde(with = "option_protocol")]
    pub jit_sandwich_most_arbed_dex_revenue: Option<Protocol>,
    pub jit_sandwich_most_arbed_dex_revenue_amt: Option<f64>,
    #[serde(with = "vec_address")]
    #[serde(rename = "jit_sandwich_arbed_pool_all.profit")]
    pub jit_sandwich_arbed_pool_all_profit: Vec<Address>,
    #[serde(rename = "jit_sandwich_arbed_pool_all.profit_amt")]
    pub jit_sandwich_arbed_pool_all_profit_amt: Vec<f64>,
    #[serde(with = "vec_address")]
    #[serde(rename = "jit_sandwich_arbed_pool_all.revenue")]
    pub jit_sandwich_arbed_pool_all_revenue: Vec<Address>,
    #[serde(rename = "jit_sandwich_arbed_pool_all.revenue_amt")]
    pub jit_sandwich_arbed_pool_all_revenue_amt: Vec<f64>,
    #[serde(rename = "jit_sandwich_arbed_pair_all.profit")]
    pub jit_sandwich_arbed_pair_all_profit: Vec<TokenPairDetails>,
    #[serde(rename = "jit_sandwich_arbed_pair_all.profit_amt")]
    pub jit_sandwich_arbed_pair_all_profit_amt: Vec<f64>,
    #[serde(rename = "jit_sandwich_arbed_pair_all.revenue")]
    pub jit_sandwich_arbed_pair_all_revenue: Vec<TokenPairDetails>,
    #[serde(rename = "jit_sandwich_arbed_pair_all.revenue_amt")]
    pub jit_sandwich_arbed_pair_all_revenue_amt: Vec<f64>,
    #[serde(rename = "jit_sandwich_arbed_dex_all.profit")]
    #[serde(with = "vec_protocol")]
    pub jit_sandwich_arbed_dex_all_profit: Vec<Protocol>,
    #[serde(rename = "jit_sandwich_arbed_dex_all.profit_amt")]
    pub jit_sandwich_arbed_dex_all_profit_amt: Vec<f64>,
    #[serde(rename = "jit_sandwich_arbed_dex_all.revenue")]
    #[serde(with = "vec_protocol")]
    pub jit_sandwich_arbed_dex_all_revenue: Vec<Protocol>,
    #[serde(rename = "jit_sandwich_arbed_dex_all.revenue_amt")]
    pub jit_sandwich_arbed_dex_all_revenue_amt: Vec<f64>,
    #[serde(with = "option_txhash")]
    pub jit_sandwich_biggest_arb_profit: Option<TxHash>,
    pub jit_sandwich_biggest_arb_profit_amt: Option<f64>,
    #[serde(with = "option_txhash")]
    pub jit_sandwich_biggest_arb_revenue: Option<TxHash>,
    pub jit_sandwich_biggest_arb_revenue_amt: Option<f64>,

    // cex dex
    pub cex_dex_bundle_count: u64,
    pub cex_dex_total_profit: f64,
    pub cex_dex_total_revenue: f64,
    pub cex_dex_average_profit_margin: f64,
    #[serde(with = "option_address")]
    pub cex_dex_top_searcher_profit: Option<Address>,
    pub cex_dex_top_searcher_profit_amt: Option<f64>,
    #[serde(with = "option_address")]
    pub cex_dex_top_searcher_revenue: Option<Address>,
    pub cex_dex_top_searcher_revenue_amt: Option<f64>,
    #[serde(with = "vec_address")]
    #[serde(rename = "cex_dex_searcher_eoa_all.profit")]
    pub cex_dex_searcher_eoa_all_profit: Vec<Address>,
    #[serde(rename = "cex_dex_searcher_eoa_all.profit_amt")]
    pub cex_dex_searcher_eoa_all_profit_amt: Vec<f64>,
    #[serde(with = "vec_address")]
    #[serde(rename = "cex_dex_searcher_eoa_all.revenue")]
    pub cex_dex_searcher_eoa_all_revenue: Vec<Address>,
    #[serde(rename = "cex_dex_searcher_eoa_all.revenue_amt")]
    pub cex_dex_searcher_eoa_all_revenue_amt: Vec<f64>,
    pub cex_dex_searcher_eoa_count: u64,
    #[serde(with = "vec_address")]
    #[serde(rename = "cex_dex_mev_contract_all.profit")]
    pub cex_dex_mev_contract_all_profit: Vec<Address>,
    #[serde(rename = "cex_dex_mev_contract_all.profit_amt")]
    pub cex_dex_mev_contract_all_profit_amt: Vec<f64>,
    #[serde(with = "vec_address")]
    #[serde(rename = "cex_dex_mev_contract_all.revenue")]
    pub cex_dex_mev_contract_all_revenue: Vec<Address>,
    #[serde(rename = "cex_dex_mev_contract_all.revenue_amt")]
    pub cex_dex_mev_contract_all_revenue_amt: Vec<f64>,
    pub cex_dex_mev_contract_count: u64,
    #[serde(with = "option_fund")]
    pub cex_dex_top_fund_profit: Option<Fund>,
    pub cex_dex_top_fund_profit_amt: Option<f64>,
    #[serde(with = "option_fund")]
    pub cex_dex_top_fund_revenue: Option<Fund>,
    pub cex_dex_top_fund_revenue_amt: Option<f64>,
    #[serde(rename = "cex_dex_fund_all.profit")]
    #[serde(with = "vec_fund")]
    pub cex_dex_fund_all_profit: Vec<Fund>,
    #[serde(rename = "cex_dex_fund_all.profit_amt")]
    pub cex_dex_fund_all_profit_amt: Vec<f64>,
    #[serde(rename = "cex_dex_fund_all.revenue")]
    #[serde(with = "vec_fund")]
    pub cex_dex_fund_all_revenue: Vec<Fund>,
    #[serde(rename = "cex_dex_fund_all.revenue_amt")]
    pub cex_dex_fund_all_revenue_amt: Vec<f64>,
    pub cex_dex_fund_count: u64,
    #[serde(with = "option_address")]
    pub cex_dex_most_arbed_pool_profit: Option<Address>,
    pub cex_dex_most_arbed_pool_profit_amt: Option<f64>,
    #[serde(with = "option_address")]
    pub cex_dex_most_arbed_pool_revenue: Option<Address>,
    pub cex_dex_most_arbed_pool_revenue_amt: Option<f64>,
    pub cex_dex_most_arbed_pair_profit: TokenPairDetails,
    pub cex_dex_most_arbed_pair_profit_amt: Option<f64>,
    pub cex_dex_most_arbed_pair_revenue: TokenPairDetails,
    pub cex_dex_most_arbed_pair_revenue_amt: Option<f64>,
    #[serde(with = "option_protocol")]
    pub cex_dex_most_arbed_dex_profit: Option<Protocol>,
    pub cex_dex_most_arbed_dex_profit_amt: Option<f64>,
    #[serde(with = "option_protocol")]
    pub cex_dex_most_arbed_dex_revenue: Option<Protocol>,
    pub cex_dex_most_arbed_dex_revenue_amt: Option<f64>,
    #[serde(with = "vec_address")]
    #[serde(rename = "cex_dex_arbed_pool_all.profit")]
    pub cex_dex_arbed_pool_all_profit: Vec<Address>,
    #[serde(rename = "cex_dex_arbed_pool_all.profit_amt")]
    pub cex_dex_arbed_pool_all_profit_amt: Vec<f64>,
    #[serde(with = "vec_address")]
    #[serde(rename = "cex_dex_arbed_pool_all.revenue")]
    pub cex_dex_arbed_pool_all_revenue: Vec<Address>,
    #[serde(rename = "cex_dex_arbed_pool_all.revenue_amt")]
    pub cex_dex_arbed_pool_all_revenue_amt: Vec<f64>,
    #[serde(rename = "cex_dex_arbed_pair_all.profit")]
    pub cex_dex_arbed_pair_all_profit: Vec<TokenPairDetails>,
    #[serde(rename = "cex_dex_arbed_pair_all.profit_amt")]
    pub cex_dex_arbed_pair_all_profit_amt: Vec<f64>,
    #[serde(rename = "cex_dex_arbed_pair_all.revenue")]
    pub cex_dex_arbed_pair_all_revenue: Vec<TokenPairDetails>,
    #[serde(rename = "cex_dex_arbed_pair_all.revenue_amt")]
    pub cex_dex_arbed_pair_all_revenue_amt: Vec<f64>,
    #[serde(rename = "cex_dex_arbed_dex_all.profit")]
    #[serde(with = "vec_protocol")]
    pub cex_dex_arbed_dex_all_profit: Vec<Protocol>,
    #[serde(rename = "cex_dex_arbed_dex_all.profit_amt")]
    pub cex_dex_arbed_dex_all_profit_amt: Vec<f64>,
    #[serde(rename = "cex_dex_arbed_dex_all.revenue")]
    #[serde(with = "vec_protocol")]
    pub cex_dex_arbed_dex_all_revenue: Vec<Protocol>,
    #[serde(rename = "cex_dex_arbed_dex_all.revenue_amt")]
    pub cex_dex_arbed_dex_all_revenue_amt: Vec<f64>,
    #[serde(with = "option_txhash")]
    pub cex_dex_biggest_arb_profit: Option<TxHash>,
    pub cex_dex_biggest_arb_profit_amt: Option<f64>,
    #[serde(with = "option_txhash")]
    pub cex_dex_biggest_arb_revenue: Option<TxHash>,
    pub cex_dex_biggest_arb_revenue_amt: Option<f64>,

    // liquidation
    pub liquidation_bundle_count: u64,
    pub liquidation_total_profit: f64,
    pub liquidation_total_revenue: f64,
    pub liquidation_average_profit_margin: f64,
    #[serde(with = "option_address")]
    pub liquidation_top_searcher_profit: Option<Address>,
    pub liquidation_top_searcher_profit_amt: Option<f64>,
    #[serde(with = "option_address")]
    pub liquidation_top_searcher_revenue: Option<Address>,
    pub liquidation_top_searcher_revenue_amt: Option<f64>,
    #[serde(with = "vec_address")]
    #[serde(rename = "liquidation_searcher_eoa_all.profit")]
    pub liquidation_searcher_eoa_all_profit: Vec<Address>,
    #[serde(rename = "liquidation_searcher_eoa_all.profit_amt")]
    pub liquidation_searcher_eoa_all_profit_amt: Vec<f64>,
    #[serde(with = "vec_address")]
    #[serde(rename = "liquidation_searcher_eoa_all.revenue")]
    pub liquidation_searcher_eoa_all_revenue: Vec<Address>,
    #[serde(rename = "liquidation_searcher_eoa_all.revenue_amt")]
    pub liquidation_searcher_eoa_all_revenue_amt: Vec<f64>,
    pub liquidation_searcher_eoa_count: u64,
    #[serde(with = "vec_address")]
    #[serde(rename = "liquidation_mev_contract_all.profit")]
    pub liquidation_mev_contract_all_profit: Vec<Address>,
    #[serde(rename = "liquidation_mev_contract_all.profit_amt")]
    pub liquidation_mev_contract_all_profit_amt: Vec<f64>,
    #[serde(with = "vec_address")]
    #[serde(rename = "liquidation_mev_contract_all.revenue")]
    pub liquidation_mev_contract_all_revenue: Vec<Address>,
    #[serde(rename = "liquidation_mev_contract_all.revenue_amt")]
    pub liquidation_mev_contract_all_revenue_amt: Vec<f64>,
    pub liquidation_mev_contract_count: u64,
    #[serde(with = "option_fund")]
    pub liquidation_top_fund_profit: Option<Fund>,
    pub liquidation_top_fund_profit_amt: Option<f64>,
    #[serde(with = "option_fund")]
    pub liquidation_top_fund_revenue: Option<Fund>,
    pub liquidation_top_fund_revenue_amt: Option<f64>,
    #[serde(rename = "liquidation_fund_all.profit")]
    #[serde(with = "vec_fund")]
    pub liquidation_fund_all_profit: Vec<Fund>,
    #[serde(rename = "liquidation_fund_all.profit_amt")]
    pub liquidation_fund_all_profit_amt: Vec<f64>,
    #[serde(rename = "liquidation_fund_all.revenue")]
    #[serde(with = "vec_fund")]
    pub liquidation_fund_all_revenue: Vec<Fund>,
    #[serde(rename = "liquidation_fund_all.revenue_amt")]
    pub liquidation_fund_all_revenue_amt: Vec<f64>,
    pub liquidation_fund_count: u64,
    pub most_liquidated_token_revenue: SingleTokenDetails,
    pub most_liquidated_token_revenue_amt: Option<f64>,
    pub most_liquidated_token_profit: SingleTokenDetails,
    pub most_liquidated_token_profit_amt: Option<f64>,
    #[serde(rename = "liquidated_tokens.revenue")]
    pub liquidated_tokens_revenue: Vec<SingleTokenDetails>,
    #[serde(rename = "liquidated_tokens.revenue_amt")]
    pub liquidated_tokens_revenue_amt: Vec<f64>,
    #[serde(rename = "liquidated_tokens.profit")]
    pub liquidated_tokens_profit: Vec<SingleTokenDetails>,
    #[serde(rename = "liquidated_tokens.profit_amt")]
    pub liquidated_tokens_profit_amt: Vec<f64>,
    #[serde(with = "option_txhash")]
    pub liquidated_biggest_arb_profit: Option<TxHash>,
    pub liquidated_biggest_arb_profit_amt: Option<f64>,
    #[serde(with = "option_txhash")]
    pub liquidated_biggest_arb_revenue: Option<TxHash>,
    pub liquidated_biggest_arb_revenue_amt: Option<f64>,
    pub total_usd_liquidated: f64,

    // builder
    pub builder_profit_usd: f64,
    pub builder_profit_eth: f64,
    pub builder_revenue_usd: f64,
    pub builder_revenue_eth: f64,
    pub builder_mev_profit_usd: f64,
    pub builder_mev_profit_eth: f64,
    pub builder_name: Option<String>,
    #[serde(with = "address")]
    pub builder_address: Address,
    pub proposer_profit_usd: Option<f64>,
    pub proposer_profit_eth: Option<f64>,
}

impl BlockAnalysis {
    #[rustfmt::skip]
    pub fn new(block: &MevBlock, bundles: &[Bundle]) -> Self {
        // All fields
        let (all_profit_addr, all_profit_am) =
            Self::top_searcher_by_profit(|b| b != MevType::SearcherTx && b!= MevType::CexDexTrades, bundles).unzip();
        let (all_rev_addr, all_rev_am) =
            Self::top_searcher_by_rev(|b| b != MevType::SearcherTx  && b!= MevType::CexDexTrades, bundles).unzip();

        let (all_biggest_tx_prof, all_biggest_prof) =
            Self::biggest_arb_profit(|b| b != MevType::SearcherTx  && b!= MevType::CexDexTrades, bundles).unzip();

        let (all_biggest_tx_rev, all_biggest_rev) =
            Self::biggest_arb_revenue(|b| b != MevType::SearcherTx  && b!= MevType::CexDexTrades, bundles).unzip();

        let (fund_rev, fund_rev_am) =
            Self::top_fund_by_type_rev(|b| b != MevType::SearcherTx && b!= MevType::CexDexTrades, bundles).unzip();
        let (fund_profit, fund_profit_am) =
            Self::top_fund_by_type_rev(|b| b != MevType::SearcherTx && b!= MevType::CexDexTrades, bundles).unzip();

        let (all_pool_addr_prof, all_pool_addr_rev, all_pool_prof, all_pool_rev) =
            Self::most_transacted_pool(
                |b| b != MevType::SearcherTx && b != MevType::Liquidation && b!= MevType::CexDexTrades,
                bundles,
                Self::get_pool_fn,
            )
            .four_unzip();

        let (all_pair_addr_prof, all_pair_addr_rev, all_pair_prof, all_pair_rev) =
            Self::most_transacted_pair(
                |b| b != MevType::SearcherTx && b != MevType::Liquidation &&  b!= MevType::CexDexTrades,
                bundles,
                Self::get_pair_fn,
            )
            .four_unzip();

        let (all_dex_addr_prof, all_dex_addr_rev, all_dex_prof, all_dex_rev) =
            Self::most_transacted_dex(
                |b| b != MevType::SearcherTx && b != MevType::Liquidation  || b!= MevType::CexDexTrades,
                bundles,
                Self::get_dex_fn,
            )
            .four_unzip();

        // Atomic Fields
        let (atomic_searcher_prof_addr, atomic_searcher_prof) =
            Self::top_searcher_by_profit(|b| b == MevType::AtomicArb, bundles).unzip();
        let (atomic_searcher_rev_addr, atomic_searcher_rev) =
            Self::top_searcher_by_rev(|b| b == MevType::AtomicArb, bundles).unzip();

        let (atomic_all_searcher_prof_addr_eoa, atomic_all_searcher_prof_eoa) =
            Self::all_searchers_by_profit(|b| b == MevType::AtomicArb, bundles, false)
                .into_iter()
                .unzip();
        let (atomic_all_searcher_rev_addr_eoa, atomic_all_searcher_rev_eoa) =
            Self::all_searchers_by_rev(|b| b == MevType::AtomicArb, bundles, false)
                .into_iter()
                .unzip();
        let (atomic_all_searcher_prof_addr_contract, atomic_all_searcher_prof_contract) =
            Self::all_searchers_by_profit(|b| b == MevType::AtomicArb, bundles, true)
                .into_iter()
                .unzip();
        let (atomic_all_searcher_rev_addr_contract, atomic_all_searcher_rev_contract) =
            Self::all_searchers_by_rev(|b| b == MevType::AtomicArb, bundles, true)
                .into_iter()
                .unzip();

        let (atomic_biggest_tx_prof, atomic_biggest_prof) =
            Self::biggest_arb_profit(|b| b == MevType::AtomicArb, bundles).unzip();

        let (atomic_biggest_tx_rev, atomic_biggest_rev) =
            Self::biggest_arb_revenue(|b| b == MevType::AtomicArb, bundles).unzip();

        let (atomic_fund_rev_addr, atomic_fund_rev) =
            Self::top_fund_by_type_rev(|b| b == MevType::AtomicArb, bundles).unzip();
        let (atomic_fund_profit_addr, atomic_fund_profit) =
            Self::top_fund_by_type_profit(|b| b == MevType::AtomicArb, bundles).unzip();

        let (atomic_all_funds_rev_addr, atomic_all_funds_rev) =
            Self::all_funds_by_type_rev(|b| b == MevType::AtomicArb, bundles)
                .into_iter()
                .unzip();
        let (atomic_all_funds_profit_addr, atomic_all_funds_profit) =
            Self::all_funds_by_type_profit(|b| b == MevType::AtomicArb, bundles)
                .into_iter()
                .unzip();

        let (atomic_pool_addr_prof, atomic_pool_addr_rev, atomic_pool_prof, atomic_pool_rev) =
            Self::most_transacted_pool(|b| b == MevType::AtomicArb, bundles, Self::get_pool_fn)
                .four_unzip();

        let (
            atomic_all_pools_addr_prof,
            atomic_all_pools_prof,
            atomic_all_pools_addr_rev,
            atomic_all_pools_rev,
        ) = Self::all_transacted_pools(|b| b == MevType::AtomicArb, bundles, Self::get_pool_fn)
            .four_unzip();

        let (
            atomic_all_pairs_addr_prof,
            atomic_all_pairs_prof,
            atomic_all_pairs_addr_rev,
            atomic_all_pairs_rev,
        ) = Self::all_transacted_pairs(|b| b == MevType::AtomicArb, bundles, Self::get_pair_fn)
            .four_unzip();

        let (
            atomic_all_dexes_addr_prof,
            atomic_all_dexes_prof,
            atomic_all_dexes_addr_rev,
            atomic_all_dexes_rev,
        ) = Self::all_transacted_dexes(|b| b == MevType::AtomicArb, bundles, Self::get_dex_fn)
            .four_unzip();

        let (atomic_pair_addr_prof, atomic_pair_addr_rev, atomic_pair_prof, atomic_pair_rev) =
            Self::most_transacted_pair(|b| b == MevType::AtomicArb, bundles, Self::get_pair_fn)
                .unwrap_or_default();
        let (atomic_dex_addr_prof, atomic_dex_addr_rev, atomic_dex_prof, atomic_dex_rev) =
            Self::most_transacted_dex(|b| b == MevType::AtomicArb, bundles, Self::get_dex_fn)
                .four_unzip();

        // Sandwich Fields
        let (sandwich_biggest_tx_prof, sandwich_biggest_prof) =
            Self::biggest_arb_profit(|b| b == MevType::Sandwich, bundles).unzip();

        let (sandwich_biggest_tx_rev, sandwich_biggest_rev) =
            Self::biggest_arb_revenue(|b| b == MevType::Sandwich, bundles).unzip();

        let (sandwich_searcher_prof_addr, sandwich_searcher_prof) =
            Self::top_searcher_by_profit(|b| b == MevType::Sandwich, bundles).unzip();
        let (sandwich_searcher_rev_addr, sandwich_searcher_rev) =
            Self::top_searcher_by_rev(|b| b == MevType::Sandwich, bundles).unzip();

        let (sandwich_all_funds_rev_addr, sandwich_all_funds_rev) =
            Self::all_funds_by_type_rev(|b| b == MevType::Sandwich, bundles)
                .into_iter()
                .unzip();
        let (sandwich_all_funds_profit_addr, sandwich_all_funds_profit) =
            Self::all_funds_by_type_profit(|b| b == MevType::Sandwich, bundles)
                .into_iter()
                .unzip();

        let (sandwich_all_searcher_prof_addr_eoa, sandwich_all_searcher_prof_eoa) =
            Self::all_searchers_by_profit(|b| b == MevType::Sandwich, bundles, false)
                .into_iter()
                .unzip();
        let (sandwich_all_searcher_rev_addr_eoa, sandwich_all_searcher_rev_eoa) =
            Self::all_searchers_by_rev(|b| b == MevType::Sandwich, bundles, false)
                .into_iter()
                .unzip();
        let (sandwich_all_searcher_prof_addr_contract, sandwich_all_searcher_prof_contract) =
            Self::all_searchers_by_profit(|b| b == MevType::Sandwich, bundles, true)
                .into_iter()
                .unzip();
        let (sandwich_all_searcher_rev_addr_contract, sandwich_all_searcher_rev_contract) =
            Self::all_searchers_by_rev(|b| b == MevType::Sandwich, bundles, true)
                .into_iter()
                .unzip();
        let (
            sandwich_pool_addr_prof,
            sandwich_pool_addr_rev,
            sandwich_pool_prof,
            sandwich_pool_rev,
        ) = Self::most_transacted_pool(|b| b == MevType::Sandwich, bundles, Self::get_pool_fn)
            .four_unzip();
        let (
            sandwich_pair_addr_prof,
            sandwich_pair_addr_rev,
            sandwich_pair_prof,
            sandwich_pair_rev,
        ) = Self::most_transacted_pair(|b| b == MevType::Sandwich, bundles, Self::get_pair_fn)
            .unwrap_or_default();
        let (sandwich_dex_addr_prof, sandwich_dex_addr_rev, sandwich_dex_prof, sandwich_dex_rev) =
            Self::most_transacted_dex(|b| b == MevType::Sandwich, bundles, Self::get_dex_fn)
                .four_unzip();

        let (sandwich_fund_rev_addr, sandwich_fund_rev) =
            Self::top_fund_by_type_rev(|b| b == MevType::Sandwich, bundles).unzip();
        let (sandwich_fund_profit_addr, sandwich_fund_profit) =
            Self::top_fund_by_type_profit(|b| b == MevType::Sandwich, bundles).unzip();

        let (
            sandwich_all_pools_addr_prof,
            sandwich_all_pools_prof,
            sandwich_all_pools_addr_rev,
            sandwich_all_pools_rev,
        ) = Self::all_transacted_pools(|b| b == MevType::Sandwich, bundles, Self::get_pool_fn)
            .four_unzip();

        let (
            sandwich_all_pairs_addr_prof,
            sandwich_all_pairs_prof,
            sandwich_all_pairs_addr_rev,
            sandwich_all_pairs_rev,
        ) = Self::all_transacted_pairs(|b| b == MevType::Sandwich, bundles, Self::get_pair_fn)
            .four_unzip();

        let (
            sandwich_all_dexes_addr_prof,
            sandwich_all_dexes_prof,
            sandwich_all_dexes_addr_rev,
            sandwich_all_dexes_rev,
        ) = Self::all_transacted_dexes(|b| b == MevType::Sandwich, bundles, Self::get_dex_fn)
            .four_unzip();

        // Jit Fields
        let (jit_searcher_prof_addr, jit_searcher_prof) =
            Self::top_searcher_by_profit(|b| b == MevType::Jit, bundles).unzip();
        let (jit_searcher_rev_addr, jit_searcher_rev) =
            Self::top_searcher_by_rev(|b| b == MevType::Jit, bundles).unzip();

        let (jit_all_searcher_prof_addr_eoa, jit_all_searcher_prof_eoa) =
            Self::all_searchers_by_profit(|b| b == MevType::Jit, bundles, false)
                .into_iter()
                .unzip();
        let (jit_all_searcher_rev_addr_eoa, jit_all_searcher_rev_eoa) =
            Self::all_searchers_by_rev(|b| b == MevType::Jit, bundles, false)
                .into_iter()
                .unzip();
        let (jit_all_searcher_prof_addr_contract, jit_all_searcher_prof_contract) =
            Self::all_searchers_by_profit(|b| b == MevType::Jit, bundles, true)
                .into_iter()
                .unzip();
        let (jit_all_searcher_rev_addr_contract, jit_all_searcher_rev_contract) =
            Self::all_searchers_by_rev(|b| b == MevType::Jit, bundles, true)
                .into_iter()
                .unzip();

        let (jit_biggest_tx_prof, jit_biggest_prof) =
            Self::biggest_arb_profit(|b| b == MevType::Jit, bundles).unzip();

        let (jit_biggest_tx_rev, jit_biggest_rev) =
            Self::biggest_arb_revenue(|b| b == MevType::Jit, bundles).unzip();

        let (jit_pool_addr_prof, jit_pool_addr_rev, jit_pool_prof, jit_pool_rev) =
            Self::most_transacted_pool(|b| b == MevType::Jit, bundles, Self::get_pool_fn)
                .four_unzip();
        let (jit_pair_addr_prof, jit_pair_addr_rev, jit_pair_prof, jit_pair_rev) =
            Self::most_transacted_pair(|b| b == MevType::Jit, bundles, Self::get_pair_fn)
                .unwrap_or_default();
        let (jit_dex_addr_prof, jit_dex_addr_rev, jit_dex_prof, jit_dex_rev) =
            Self::most_transacted_dex(|b| b == MevType::Jit, bundles, Self::get_dex_fn)
                .four_unzip();

        let (jit_fund_rev_addr, jit_fund_rev) =
            Self::top_fund_by_type_rev(|b| b == MevType::Jit, bundles).unzip();
        let (jit_fund_profit_addr, jit_fund_profit) =
            Self::top_fund_by_type_profit(|b| b == MevType::Jit, bundles).unzip();

        let (jit_all_funds_rev_addr, jit_all_funds_rev) =
            Self::all_funds_by_type_rev(|b| b == MevType::Jit, bundles)
                .into_iter()
                .unzip();
        let (jit_all_funds_profit_addr, jit_all_funds_profit) =
            Self::all_funds_by_type_profit(|b| b == MevType::Jit, bundles)
                .into_iter()
                .unzip();

        let (
            jit_all_pools_addr_prof,
            jit_all_pools_prof,
            jit_all_pools_addr_rev,
            jit_all_pools_rev,
        ) = Self::all_transacted_pools(|b| b == MevType::Jit, bundles, Self::get_pool_fn)
            .four_unzip();

        let (
            jit_all_pairs_addr_prof,
            jit_all_pairs_prof,
            jit_all_pairs_addr_rev,
            jit_all_pairs_rev,
        ) = Self::all_transacted_pairs(|b| b == MevType::Jit, bundles, Self::get_pair_fn)
            .four_unzip();

        let (
            jit_all_dexes_addr_prof,
            jit_all_dexes_prof,
            jit_all_dexes_addr_rev,
            jit_all_dexes_rev,
        ) = Self::all_transacted_dexes(|b| b == MevType::Jit, bundles, Self::get_dex_fn)
            .four_unzip();

        // Jit Sando Fields
        let (jit_sandwich_biggest_tx_prof, jit_sandwich_biggest_prof) =
            Self::biggest_arb_profit(|b| b == MevType::JitSandwich, bundles).unzip();
        let (jit_sandwich_biggest_tx_rev, jit_sandwich_biggest_rev) =
            Self::biggest_arb_revenue(|b| b == MevType::JitSandwich, bundles).unzip();
        let (jit_sandwich_searcher_prof_addr, jit_sandwich_searcher_prof) =
            Self::top_searcher_by_profit(|b| b == MevType::JitSandwich, bundles).unzip();
        let (jit_sandwich_searcher_rev_addr, jit_sandwich_searcher_rev) =
            Self::top_searcher_by_rev(|b| b == MevType::JitSandwich, bundles).unzip();

        let (jit_sandwich_all_searcher_prof_addr_eoa, jit_sandwich_all_searcher_prof_eoa) =
            Self::all_searchers_by_profit(|b| b == MevType::JitSandwich, bundles, false)
                .into_iter()
                .unzip();
        let (jit_sandwich_all_searcher_rev_addr_eoa, jit_sandwich_all_searcher_rev_eoa) =
            Self::all_searchers_by_rev(|b| b == MevType::JitSandwich, bundles, false)
                .into_iter()
                .unzip();
        let (jit_sandwich_all_searcher_prof_addr_contract, jit_sandwich_all_searcher_prof_contract) =
            Self::all_searchers_by_profit(|b| b == MevType::JitSandwich, bundles, true)
                .into_iter()
                .unzip();
        let (jit_sandwich_all_searcher_rev_addr_contract, jit_sandwich_all_searcher_rev_contract) =
            Self::all_searchers_by_rev(|b| b == MevType::JitSandwich, bundles, true)
                .into_iter()
                .unzip();

        let (jit_sandwich_all_funds_rev_addr, jit_sandwich_all_funds_rev) =
            Self::all_funds_by_type_rev(|b| b == MevType::JitSandwich, bundles)
                .into_iter()
                .unzip();
        let (jit_sandwich_all_funds_profit_addr, jit_sandwich_all_funds_profit) =
            Self::all_funds_by_type_profit(|b| b == MevType::JitSandwich, bundles)
                .into_iter()
                .unzip();

        let (
            jit_sandwich_pool_addr_prof,
            jit_sandwich_pool_addr_rev,
            jit_sandwich_pool_prof,
            jit_sandwich_pool_rev,
        ) = Self::most_transacted_pool(|b| b == MevType::JitSandwich, bundles, Self::get_pool_fn)
            .four_unzip();
        let (
            jit_sandwich_pair_addr_prof,
            jit_sandwich_pair_addr_rev,
            jit_sandwich_pair_prof,
            jit_sandwich_pair_rev,
        ) = Self::most_transacted_pair(|b| b == MevType::JitSandwich, bundles, Self::get_pair_fn)
            .unwrap_or_default();
        let (
            jit_sandwich_dex_addr_prof,
            jit_sandwich_dex_addr_rev,
            jit_sandwich_dex_prof,
            jit_sandwich_dex_rev,
        ) = Self::most_transacted_dex(|b| b == MevType::JitSandwich, bundles, Self::get_dex_fn)
            .four_unzip();

        let (jit_sandwich_fund_rev_addr, jit_sandwich_fund_rev) =
            Self::top_fund_by_type_rev(|b| b == MevType::JitSandwich, bundles).unzip();
        let (jit_sandwich_fund_profit_addr, jit_sandwich_fund_profit) =
            Self::top_fund_by_type_profit(|b| b == MevType::JitSandwich, bundles).unzip();

        let (
            jit_sandwich_all_pools_addr_prof,
            jit_sandwich_all_pools_prof,
            jit_sandwich_all_pools_addr_rev,
            jit_sandwich_all_pools_rev,
        ) = Self::all_transacted_pools(|b| b == MevType::JitSandwich, bundles, Self::get_pool_fn)
            .four_unzip();

        let (
            jit_sandwich_all_pairs_addr_prof,
            jit_sandwich_all_pairs_prof,
            jit_sandwich_all_pairs_addr_rev,
            jit_sandwich_all_pairs_rev,
        ) = Self::all_transacted_pairs(|b| b == MevType::JitSandwich, bundles, Self::get_pair_fn)
            .four_unzip();

        let (
            jit_sandwich_all_dexes_addr_prof,
            jit_sandwich_all_dexes_prof,
            jit_sandwich_all_dexes_addr_rev,
            jit_sandwich_all_dexes_rev,
        ) = Self::all_transacted_dexes(|b| b == MevType::JitSandwich, bundles, Self::get_dex_fn)
            .four_unzip();

        // Cex Dex
        let (cex_dex_searcher_prof_addr, cex_dex_searcher_prof) =
            Self::top_searcher_by_profit(|b| b == MevType::CexDexQuotes, bundles).unzip();
        let (cex_dex_searcher_rev_addr, cex_dex_searcher_rev) =
            Self::top_searcher_by_rev(|b| b == MevType::CexDexQuotes, bundles).unzip();

        let (cex_dex_biggest_tx_prof, cex_dex_biggest_prof) =
            Self::biggest_arb_profit(|b| b == MevType::CexDexQuotes, bundles).unzip();

        let (cex_dex_biggest_tx_rev, cex_dex_biggest_rev) =
            Self::biggest_arb_revenue(|b| b == MevType::CexDexQuotes, bundles).unzip();

        let (cex_dex_all_funds_rev_addr, cex_dex_all_funds_rev) =
            Self::all_funds_by_type_rev(|b| b == MevType::CexDexQuotes, bundles)
                .into_iter()
                .unzip();
        let (cex_dex_all_funds_profit_addr, cex_dex_all_funds_profit) =
            Self::all_funds_by_type_profit(|b| b == MevType::CexDexQuotes, bundles)
                .into_iter()
                .unzip();

        let (cex_dex_all_searcher_prof_addr_eoa, cex_dex_all_searcher_prof_eoa) =
            Self::all_searchers_by_profit(|b| b == MevType::CexDexQuotes, bundles, false)
                .into_iter()
                .unzip();
        let (cex_dex_all_searcher_rev_addr_eoa, cex_dex_all_searcher_rev_eoa) =
            Self::all_searchers_by_rev(|b| b == MevType::CexDexQuotes, bundles, false)
                .into_iter()
                .unzip();
        let (cex_dex_all_searcher_prof_addr_contract, cex_dex_all_searcher_prof_contract) =
            Self::all_searchers_by_profit(|b| b == MevType::CexDexQuotes, bundles, true)
                .into_iter()
                .unzip();
        let (cex_dex_all_searcher_rev_addr_contract, cex_dex_all_searcher_rev_contract) =
            Self::all_searchers_by_rev(|b| b == MevType::CexDexQuotes, bundles, true)
                .into_iter()
                .unzip();

        let (cex_dex_fund_rev_addr, cex_dex_fund_rev) =
            Self::top_fund_by_type_rev(|b| b == MevType::CexDexQuotes, bundles).unzip();
        let (cex_dex_fund_profit_addr, cex_dex_fund_profit) =
            Self::top_fund_by_type_profit(|b| b == MevType::CexDexQuotes, bundles).unzip();

        let (cex_dex_pool_addr_prof, cex_dex_pool_addr_rev, cex_dex_pool_prof, cex_dex_pool_rev) =
            Self::most_transacted_pool(|b| b == MevType::CexDexQuotes, bundles, Self::get_pool_fn)
                .four_unzip();
        let (cex_dex_pair_addr_prof, cex_dex_pair_addr_rev, cex_dex_pair_prof, cex_dex_pair_rev) =
            Self::most_transacted_pair(|b| b == MevType::CexDexQuotes, bundles, Self::get_pair_fn)
                .unwrap_or_default();
        let (cex_dex_dex_addr_prof, cex_dex_dex_addr_rev, cex_dex_dex_prof, cex_dex_dex_rev) =
            Self::most_transacted_dex(|b| b == MevType::CexDexQuotes, bundles, Self::get_dex_fn)
                .four_unzip();

        let (
            cex_dex_all_pools_addr_prof,
            cex_dex_all_pools_prof,
            cex_dex_all_pools_addr_rev,
            cex_dex_all_pools_rev,
        ) = Self::all_transacted_pools(|b| b == MevType::CexDexQuotes, bundles, Self::get_pool_fn)
            .four_unzip();

        let (
            cex_dex_all_pairs_addr_prof,
            cex_dex_all_pairs_prof,
            cex_dex_all_pairs_addr_rev,
            cex_dex_all_pairs_rev,
        ) = Self::all_transacted_pairs(|b| b == MevType::CexDexQuotes, bundles, Self::get_pair_fn)
            .four_unzip();

        let (
            cex_dex_all_dexes_addr_prof,
            cex_dex_all_dexes_prof,
            cex_dex_all_dexes_addr_rev,
            cex_dex_all_dexes_rev,
        ) = Self::all_transacted_dexes(|b| b == MevType::CexDexQuotes, bundles, Self::get_dex_fn)
            .four_unzip();

        // liquidation
        let (liquidation_searcher_prof_addr, liquidation_searcher_prof) =
            Self::top_searcher_by_profit(|b| b == MevType::Liquidation, bundles).unzip();
        let (liquidation_searcher_rev_addr, liquidation_searcher_rev) =
            Self::top_searcher_by_rev(|b| b == MevType::Liquidation, bundles).unzip();

        let (liq_most_token_prof, liq_most_token_rev, liq_most_prof, liq_most_rev) =
            Self::most_transacted(
                |b| b == MevType::Liquidation,
                bundles,
                |data| {
                    let BundleData::Liquidation(l) = data else { unreachable!() };
                    l.liquidations
                        .iter()
                        .map(|l| l.collateral_asset.clone().into())
                        .collect::<Vec<_>>()
                },
            )
            .four_unzip();

        let (liq_all_token_prof, liq_all_prof, liq_all_token_rev, liq_all_rev) =
            Self::all_transacted(
                |b| b == MevType::Liquidation,
                bundles,
                |data| {
                    let BundleData::Liquidation(l) = data else { unreachable!() };
                    l.liquidations
                        .iter()
                        .map(|l| l.collateral_asset.clone().into())
                        .collect::<Vec<_>>()
                },
            )
            .four_unzip();

        let (liquidation_fund_rev_addr, liquidation_fund_rev) =
            Self::top_fund_by_type_rev(|b| b == MevType::Liquidation, bundles).unzip();
        let (liquidation_fund_profit_addr, liquidation_fund_profit) =
            Self::top_fund_by_type_profit(|b| b == MevType::Liquidation, bundles).unzip();

        let (liquidation_all_searcher_prof_addr_eoa, liquidation_all_searcher_prof_eoa) =
            Self::all_searchers_by_profit(|b| b == MevType::Liquidation, bundles, false)
                .into_iter()
                .unzip();
        let (liquidation_all_searcher_rev_addr_eoa, liquidation_all_searcher_rev_eoa) =
            Self::all_searchers_by_rev(|b| b == MevType::Liquidation, bundles, false)
                .into_iter()
                .unzip();
        let (liquidation_all_searcher_prof_addr_contract, liquidation_all_searcher_prof_contract) =
            Self::all_searchers_by_profit(|b| b == MevType::Liquidation, bundles, true)
                .into_iter()
                .unzip();
        let (liquidation_all_searcher_rev_addr_contract, liquidation_all_searcher_rev_contract) =
            Self::all_searchers_by_rev(|b| b == MevType::Liquidation, bundles, true)
                .into_iter()
                .unzip();

        let (liquidation_all_funds_rev_addr, liquidation_all_funds_rev) =
            Self::all_funds_by_type_rev(|b| b == MevType::Liquidation, bundles)
                .into_iter()
                .unzip();
        let (liquidation_all_funds_profit_addr, liquidation_all_funds_profit) =
            Self::all_funds_by_type_profit(|b| b == MevType::Liquidation, bundles)
                .into_iter()
                .unzip();

        let (liquidation_biggest_tx_prof, liquidation_biggest_prof) =
            Self::biggest_arb_profit(|b| b == MevType::Liquidation, bundles).unzip();

        let (liquidation_biggest_tx_rev, liquidation_biggest_rev) =
            Self::biggest_arb_revenue(|b| b == MevType::Liquidation, bundles).unzip();

        Self {
            block_number: block.block_number,
            eth_price: block.eth_price,
            all_bundle_count: Self::total_count_by_type(|f| f != MevType::SearcherTx, bundles),
            all_total_profit: Self::total_profit_by_type(|f| f != MevType::SearcherTx, bundles),
            all_total_revenue: Self::total_revenue_by_type(|f| f != MevType::SearcherTx, bundles),
            all_average_profit_margin: Self::average_profit_margin(
                |f| f != MevType::SearcherTx,
                bundles,
            )
            .unwrap_or_default(),
            all_searcher_count: Self::unique_eoa(|b| b != MevType::SearcherTx, bundles),
            all_top_searcher_revenue: all_rev_addr,
            all_top_searcher_revenue_amt: all_rev_am,
            all_top_searcher_profit: all_profit_addr,
            all_top_searcher_profit_amt: all_profit_am,
            all_top_fund_revenue: fund_rev,
            all_top_fund_revenue_amt: fund_rev_am,
            all_top_fund_profit_amt: fund_profit_am,
            all_top_fund_profit: fund_profit,
            all_fund_count: Self::unique_funds(|b| b != MevType::SearcherTx, bundles),
            all_most_arbed_pool_profit: all_pool_addr_prof,
            all_most_arbed_pool_profit_amt: all_pool_prof,
            all_most_arbed_dex_revenue: all_dex_addr_rev,
            all_most_arbed_pair_revenue: all_pair_addr_rev.unwrap_or_default(),
            all_most_arbed_pair_profit: all_pair_addr_prof.unwrap_or_default(),
            all_most_arbed_dex_profit_amt: all_dex_prof,
            all_most_arbed_dex_profit: all_dex_addr_prof,
            all_most_arbed_dex_revenue_amt: all_dex_rev,
            all_most_arbed_pool_revenue: all_pool_addr_rev,
            all_most_arbed_pool_revenue_amt: all_pool_rev,
            all_most_arbed_pair_revenue_amt: all_pair_rev,
            all_most_arbed_pair_profit_amt: all_pair_prof,
            all_biggest_arb_profit: all_biggest_tx_prof,
            all_biggest_arb_profit_amt: all_biggest_prof,
            all_biggest_arb_revenue: all_biggest_tx_rev,
            all_biggest_arb_revenue_amt: all_biggest_rev,

            // atomic
            atomic_bundle_count:                 Self::total_count_by_type(
                |b| b == MevType::AtomicArb,
                bundles,
            ),
            atomic_fund_count:                   Self::unique_funds(
                |b| b == MevType::AtomicArb,
                bundles,
            ),
            atomic_total_profit:                 Self::total_profit_by_type(
                |b| b == MevType::AtomicArb,
                bundles,
            ),
            atomic_total_revenue:                Self::total_revenue_by_type(
                |b| b == MevType::AtomicArb,
                bundles,
            ),
            atomic_top_searcher_profit:          atomic_searcher_prof_addr,
            atomic_top_searcher_revenue:         atomic_searcher_rev_addr,
            atomic_top_searcher_profit_amt:      atomic_searcher_prof,
            atomic_top_searcher_revenue_amt:     atomic_searcher_rev,
            atomic_top_fund_profit_amt:          atomic_fund_profit,
            atomic_top_fund_profit:              atomic_fund_profit_addr,
            atomic_top_fund_revenue:             atomic_fund_rev_addr,
            atomic_top_fund_revenue_amt:         atomic_fund_rev,
            atomic_most_arbed_dex_profit_amt:    atomic_dex_prof,
            atomic_most_arbed_dex_profit:        atomic_dex_addr_prof,
            atomic_most_arbed_dex_revenue:       atomic_dex_addr_rev,
            atomic_most_arbed_dex_revenue_amt:   atomic_dex_rev,
            atomic_most_arbed_pair_profit_amt:   Some(atomic_pair_prof),
            atomic_most_arbed_pair_profit:       atomic_pair_addr_prof,
            atomic_most_arbed_pair_revenue:      atomic_pair_addr_rev,
            atomic_most_arbed_pair_revenue_amt:  Some(atomic_pair_rev),
            atomic_most_arbed_pool_revenue_amt:  atomic_pool_rev,
            atomic_most_arbed_pool_profit_amt:   atomic_pool_prof,
            atomic_most_arbed_pool_revenue:      atomic_pool_addr_rev,
            atomic_most_arbed_pool_profit:       atomic_pool_addr_prof,
            atomic_average_profit_margin:        Self::average_profit_margin(
                |f| f == MevType::AtomicArb,
                bundles,
            )
            .unwrap_or_default(),
            atomic_biggest_arb_profit:           atomic_biggest_tx_prof,
            atomic_biggest_arb_profit_amt:       atomic_biggest_prof,
            atomic_biggest_arb_revenue:          atomic_biggest_tx_rev,
            atomic_biggest_arb_revenue_amt:      atomic_biggest_rev,
            atomic_searcher_eoa_all_profit:      atomic_all_searcher_prof_addr_eoa,
            atomic_searcher_eoa_all_profit_amt:  atomic_all_searcher_prof_eoa,
            atomic_searcher_eoa_all_revenue:     atomic_all_searcher_rev_addr_eoa,
            atomic_searcher_eoa_all_revenue_amt: atomic_all_searcher_rev_eoa,
            atomic_searcher_eoa_count:           Self::unique_eoa(
                |b| b == MevType::AtomicArb,
                bundles,
            ),
            atomic_mev_contract_all_profit:      atomic_all_searcher_prof_addr_contract,
            atomic_mev_contract_all_profit_amt:  atomic_all_searcher_prof_contract,
            atomic_mev_contract_all_revenue:     atomic_all_searcher_rev_addr_contract,
            atomic_mev_contract_all_revenue_amt: atomic_all_searcher_rev_contract,
            atomic_mev_contract_count:           Self::unique_contract(
                |b| b == MevType::AtomicArb,
                bundles,
            ),
            atomic_fund_all_profit:              atomic_all_funds_profit_addr,
            atomic_fund_all_profit_amt:          atomic_all_funds_profit,
            atomic_fund_all_revenue:             atomic_all_funds_rev_addr,
            atomic_fund_all_revenue_amt:         atomic_all_funds_rev,
            atomic_arbed_dex_all_profit:         atomic_all_dexes_addr_prof,
            atomic_arbed_dex_all_profit_amt:     atomic_all_dexes_prof,
            atomic_arbed_dex_all_revenue:        atomic_all_dexes_addr_rev,
            atomic_arbed_dex_all_revenue_amt:    atomic_all_dexes_rev,
            atomic_arbed_pair_all_profit:        atomic_all_pairs_addr_prof,
            atomic_arbed_pair_all_profit_amt:    atomic_all_pairs_prof,
            atomic_arbed_pair_all_revenue:       atomic_all_pairs_addr_rev,
            atomic_arbed_pair_all_revenue_amt:   atomic_all_pairs_rev,
            atomic_arbed_pool_all_profit:        atomic_all_pools_addr_prof,
            atomic_arbed_pool_all_profit_amt:    atomic_all_pools_prof,
            atomic_arbed_pool_all_revenue:       atomic_all_pools_addr_rev,
            atomic_arbed_pool_all_revenue_amt:   atomic_all_pools_rev,

            // sandwich
            sandwich_bundle_count:                 Self::total_count_by_type(
                |b| b == MevType::Sandwich,
                bundles,
            ),
            sandwich_total_profit:                 Self::total_profit_by_type(
                |b| b == MevType::Sandwich,
                bundles,
            ),
            sandwich_total_revenue:                Self::total_revenue_by_type(
                |b| b == MevType::Sandwich,
                bundles,
            ),
            sandwich_biggest_arb_profit_amt:       sandwich_biggest_prof,
            sandwich_biggest_arb_profit:           sandwich_biggest_tx_prof,
            sandwich_biggest_arb_revenue_amt:      sandwich_biggest_rev,
            sandwich_biggest_arb_revenue:          sandwich_biggest_tx_rev,
            sandwich_top_searcher_profit:          sandwich_searcher_prof_addr,
            sandwich_top_searcher_revenue:         sandwich_searcher_rev_addr,
            sandwich_top_searcher_profit_amt:      sandwich_searcher_prof,
            sandwich_top_searcher_revenue_amt:     sandwich_searcher_rev,
            sandwich_most_arbed_dex_profit_amt:    sandwich_dex_prof,
            sandwich_most_arbed_dex_profit:        sandwich_dex_addr_prof,
            sandwich_most_arbed_dex_revenue:       sandwich_dex_addr_rev,
            sandwich_most_arbed_dex_revenue_amt:   sandwich_dex_rev,
            sandwich_most_arbed_pair_profit_amt:   Some(sandwich_pair_prof),
            sandwich_most_arbed_pair_profit:       sandwich_pair_addr_prof,
            sandwich_most_arbed_pair_revenue:      sandwich_pair_addr_rev,
            sandwich_most_arbed_pair_revenue_amt:  Some(sandwich_pair_rev),
            sandwich_most_arbed_pool_revenue_amt:  sandwich_pool_rev,
            sandwich_most_arbed_pool_profit_amt:   sandwich_pool_prof,
            sandwich_most_arbed_pool_profit:       sandwich_pool_addr_prof,
            sandwich_most_arbed_pool_revenue:      sandwich_pool_addr_rev,
            sandwich_average_profit_margin:        Self::average_profit_margin(
                |f| f == MevType::Sandwich,
                bundles,
            )
            .unwrap_or_default(),
            sandwich_fund_count:                   Self::unique_funds(
                |b| b == MevType::Sandwich,
                bundles,
            ),
            sandwich_top_fund_profit_amt:          sandwich_fund_profit,
            sandwich_top_fund_profit:              sandwich_fund_profit_addr,
            sandwich_top_fund_revenue:             sandwich_fund_rev_addr,
            sandwich_top_fund_revenue_amt:         sandwich_fund_rev,
            sandwich_searcher_eoa_all_profit:      sandwich_all_searcher_prof_addr_eoa,
            sandwich_searcher_eoa_all_profit_amt:  sandwich_all_searcher_prof_eoa,
            sandwich_searcher_eoa_all_revenue:     sandwich_all_searcher_rev_addr_eoa,
            sandwich_searcher_eoa_all_revenue_amt: sandwich_all_searcher_rev_eoa,
            sandwich_searcher_eoa_count:           Self::unique_eoa(
                |b| b == MevType::Sandwich,
                bundles,
            ),
            sandwich_mev_contract_all_profit:      sandwich_all_searcher_prof_addr_contract,
            sandwich_mev_contract_all_profit_amt:  sandwich_all_searcher_prof_contract,
            sandwich_mev_contract_all_revenue:     sandwich_all_searcher_rev_addr_contract,
            sandwich_mev_contract_all_revenue_amt: sandwich_all_searcher_rev_contract,
            sandwich_mev_contract_count:           Self::unique_contract(
                |b| b == MevType::Sandwich,
                bundles,
            ),
            sandwich_fund_all_profit:              sandwich_all_funds_profit_addr,
            sandwich_fund_all_profit_amt:          sandwich_all_funds_profit,
            sandwich_fund_all_revenue:             sandwich_all_funds_rev_addr,
            sandwich_fund_all_revenue_amt:         sandwich_all_funds_rev,
            sandwich_arbed_dex_all_profit:         sandwich_all_dexes_addr_prof,
            sandwich_arbed_dex_all_profit_amt:     sandwich_all_dexes_prof,
            sandwich_arbed_dex_all_revenue:        sandwich_all_dexes_addr_rev,
            sandwich_arbed_dex_all_revenue_amt:    sandwich_all_dexes_rev,
            sandwich_arbed_pair_all_profit:        sandwich_all_pairs_addr_prof,
            sandwich_arbed_pair_all_profit_amt:    sandwich_all_pairs_prof,
            sandwich_arbed_pair_all_revenue:       sandwich_all_pairs_addr_rev,
            sandwich_arbed_pair_all_revenue_amt:   sandwich_all_pairs_rev,
            sandwich_arbed_pool_all_profit:        sandwich_all_pools_addr_prof,
            sandwich_arbed_pool_all_profit_amt:    sandwich_all_pools_prof,
            sandwich_arbed_pool_all_revenue:       sandwich_all_pools_addr_rev,
            sandwich_arbed_pool_all_revenue_amt:   sandwich_all_pools_rev,

            // jit
            jit_bundle_count:                 Self::total_count_by_type(
                |b| b == MevType::Jit,
                bundles,
            ),
            jit_fund_count:                   Self::unique_funds(|b| b == MevType::Jit, bundles),
            jit_total_profit:                 Self::total_profit_by_type(
                |b| b == MevType::Jit,
                bundles,
            ),
            jit_total_revenue:                Self::total_revenue_by_type(
                |b| b == MevType::Jit,
                bundles,
            ),
            jit_top_searcher_profit:          jit_searcher_prof_addr,
            jit_top_searcher_revenue:         jit_searcher_rev_addr,
            jit_top_searcher_profit_amt:      jit_searcher_prof,
            jit_top_searcher_revenue_amt:     jit_searcher_rev,
            jit_most_arbed_dex_profit_amt:    jit_dex_prof,
            jit_most_arbed_dex_profit:        jit_dex_addr_prof,
            jit_most_arbed_dex_revenue:       jit_dex_addr_rev,
            jit_most_arbed_dex_revenue_amt:   jit_dex_rev,
            jit_most_arbed_pair_profit_amt:   Some(jit_pair_prof),
            jit_most_arbed_pair_profit:       jit_pair_addr_prof,
            jit_most_arbed_pair_revenue:      jit_pair_addr_rev,
            jit_most_arbed_pair_revenue_amt:  Some(jit_pair_rev),
            jit_most_arbed_pool_revenue_amt:  jit_pool_rev,
            jit_most_arbed_pool_profit_amt:   jit_pool_prof,
            jit_most_arbed_pool_profit:       jit_pool_addr_prof,
            jit_most_arbed_pool_revenue:      jit_pool_addr_rev,
            jit_average_profit_margin:        Self::average_profit_margin(
                |f| f == MevType::Jit,
                bundles,
            )
            .unwrap_or_default(),
            jit_biggest_arb_profit:           jit_biggest_tx_prof,
            jit_biggest_arb_profit_amt:       jit_biggest_prof,
            jit_biggest_arb_revenue:          jit_biggest_tx_rev,
            jit_biggest_arb_revenue_amt:      jit_biggest_rev,
            jit_top_fund_profit_amt:          jit_fund_profit,
            jit_top_fund_profit:              jit_fund_profit_addr,
            jit_top_fund_revenue:             jit_fund_rev_addr,
            jit_top_fund_revenue_amt:         jit_fund_rev,
            jit_searcher_eoa_all_profit:      jit_all_searcher_prof_addr_eoa,
            jit_searcher_eoa_all_profit_amt:  jit_all_searcher_prof_eoa,
            jit_searcher_eoa_all_revenue:     jit_all_searcher_rev_addr_eoa,
            jit_searcher_eoa_all_revenue_amt: jit_all_searcher_rev_eoa,
            jit_searcher_eoa_count:           Self::unique_eoa(|b| b == MevType::Jit, bundles),
            jit_mev_contract_all_profit:      jit_all_searcher_prof_addr_contract,
            jit_mev_contract_all_profit_amt:  jit_all_searcher_prof_contract,
            jit_mev_contract_all_revenue:     jit_all_searcher_rev_addr_contract,
            jit_mev_contract_all_revenue_amt: jit_all_searcher_rev_contract,
            jit_mev_contract_count:           Self::unique_contract(|b| b == MevType::Jit, bundles),
            jit_fund_all_profit:              jit_all_funds_profit_addr,
            jit_fund_all_profit_amt:          jit_all_funds_profit,
            jit_fund_all_revenue:             jit_all_funds_rev_addr,
            jit_fund_all_revenue_amt:         jit_all_funds_rev,
            jit_arbed_dex_all_profit:         jit_all_dexes_addr_prof,
            jit_arbed_dex_all_profit_amt:     jit_all_dexes_prof,
            jit_arbed_dex_all_revenue:        jit_all_dexes_addr_rev,
            jit_arbed_dex_all_revenue_amt:    jit_all_dexes_rev,
            jit_arbed_pair_all_profit:        jit_all_pairs_addr_prof,
            jit_arbed_pair_all_profit_amt:    jit_all_pairs_prof,
            jit_arbed_pair_all_revenue:       jit_all_pairs_addr_rev,
            jit_arbed_pair_all_revenue_amt:   jit_all_pairs_rev,
            jit_arbed_pool_all_profit:        jit_all_pools_addr_prof,
            jit_arbed_pool_all_profit_amt:    jit_all_pools_prof,
            jit_arbed_pool_all_revenue:       jit_all_pools_addr_rev,
            jit_arbed_pool_all_revenue_amt:   jit_all_pools_rev,

            // jit sando
            jit_sandwich_bundle_count: Self::total_count_by_type(
                |b| b == MevType::JitSandwich,
                bundles,
            ),

            jit_sandwich_average_profit_margin:        Self::average_profit_margin(
                |f| f == MevType::JitSandwich,
                bundles,
            )
            .unwrap_or_default(),
            jit_sandwich_total_profit:                 Self::total_profit_by_type(
                |b| b == MevType::JitSandwich,
                bundles,
            ),
            jit_sandwich_total_revenue:                Self::total_revenue_by_type(
                |b| b == MevType::JitSandwich,
                bundles,
            ),
            jit_sandwich_top_searcher_profit:          jit_sandwich_searcher_prof_addr,
            jit_sandwich_top_searcher_revenue:         jit_sandwich_searcher_rev_addr,
            jit_sandwich_top_searcher_profit_amt:      jit_sandwich_searcher_prof,
            jit_sandwich_top_searcher_revenue_amt:     jit_sandwich_searcher_rev,
            jit_sandwich_most_arbed_dex_profit_amt:    jit_sandwich_dex_prof,
            jit_sandwich_most_arbed_dex_profit:        jit_sandwich_dex_addr_prof,
            jit_sandwich_most_arbed_dex_revenue:       jit_sandwich_dex_addr_rev,
            jit_sandwich_most_arbed_dex_revenue_amt:   jit_sandwich_dex_rev,
            jit_sandwich_most_arbed_pair_profit_amt:   Some(jit_sandwich_pair_prof),
            jit_sandwich_most_arbed_pair_profit:       jit_sandwich_pair_addr_prof,
            jit_sandwich_most_arbed_pair_revenue:      jit_sandwich_pair_addr_rev,
            jit_sandwich_most_arbed_pair_revenue_amt:  Some(jit_sandwich_pair_rev),
            jit_sandwich_most_arbed_pool_revenue_amt:  jit_sandwich_pool_rev,
            jit_sandwich_most_arbed_pool_profit_amt:   jit_sandwich_pool_prof,
            jit_sandwich_most_arbed_pool_profit:       jit_sandwich_pool_addr_prof,
            jit_sandwich_most_arbed_pool_revenue:      jit_sandwich_pool_addr_rev,
            jit_sandwich_biggest_arb_profit_amt:       jit_sandwich_biggest_prof,
            jit_sandwich_biggest_arb_profit:           jit_sandwich_biggest_tx_prof,
            jit_sandwich_biggest_arb_revenue_amt:      jit_sandwich_biggest_rev,
            jit_sandwich_biggest_arb_revenue:          jit_sandwich_biggest_tx_rev,
            jit_sandwich_fund_count:                   Self::unique_funds(
                |b| b == MevType::JitSandwich,
                bundles,
            ),
            jit_sandwich_top_fund_profit_amt:          jit_sandwich_fund_profit,
            jit_sandwich_top_fund_profit:              jit_sandwich_fund_profit_addr,
            jit_sandwich_top_fund_revenue:             jit_sandwich_fund_rev_addr,
            jit_sandwich_top_fund_revenue_amt:         jit_sandwich_fund_rev,
            jit_sandwich_searcher_eoa_all_profit:      jit_sandwich_all_searcher_prof_addr_eoa,
            jit_sandwich_searcher_eoa_all_profit_amt:  jit_sandwich_all_searcher_prof_eoa,
            jit_sandwich_searcher_eoa_all_revenue:     jit_sandwich_all_searcher_rev_addr_eoa,
            jit_sandwich_searcher_eoa_all_revenue_amt: jit_sandwich_all_searcher_rev_eoa,
            jit_sandwich_searcher_eoa_count:           Self::unique_eoa(
                |b| b == MevType::JitSandwich,
                bundles,
            ),
            jit_sandwich_mev_contract_all_profit:      jit_sandwich_all_searcher_prof_addr_contract,
            jit_sandwich_mev_contract_all_profit_amt:  jit_sandwich_all_searcher_prof_contract,
            jit_sandwich_mev_contract_all_revenue:     jit_sandwich_all_searcher_rev_addr_contract,
            jit_sandwich_mev_contract_all_revenue_amt: jit_sandwich_all_searcher_rev_contract,
            jit_sandwich_mev_contract_count:           Self::unique_contract(
                |b| b == MevType::JitSandwich,
                bundles,
            ),
            jit_sandwich_fund_all_profit:              jit_sandwich_all_funds_profit_addr,
            jit_sandwich_fund_all_profit_amt:          jit_sandwich_all_funds_profit,
            jit_sandwich_fund_all_revenue:             jit_sandwich_all_funds_rev_addr,
            jit_sandwich_fund_all_revenue_amt:         jit_sandwich_all_funds_rev,
            jit_sandwich_arbed_dex_all_profit:         jit_sandwich_all_dexes_addr_prof,
            jit_sandwich_arbed_dex_all_profit_amt:     jit_sandwich_all_dexes_prof,
            jit_sandwich_arbed_dex_all_revenue:        jit_sandwich_all_dexes_addr_rev,
            jit_sandwich_arbed_dex_all_revenue_amt:    jit_sandwich_all_dexes_rev,
            jit_sandwich_arbed_pair_all_profit:        jit_sandwich_all_pairs_addr_prof,
            jit_sandwich_arbed_pair_all_profit_amt:    jit_sandwich_all_pairs_prof,
            jit_sandwich_arbed_pair_all_revenue:       jit_sandwich_all_pairs_addr_rev,
            jit_sandwich_arbed_pair_all_revenue_amt:   jit_sandwich_all_pairs_rev,
            jit_sandwich_arbed_pool_all_profit:        jit_sandwich_all_pools_addr_prof,
            jit_sandwich_arbed_pool_all_profit_amt:    jit_sandwich_all_pools_prof,
            jit_sandwich_arbed_pool_all_revenue:       jit_sandwich_all_pools_addr_rev,
            jit_sandwich_arbed_pool_all_revenue_amt:   jit_sandwich_all_pools_rev,

            // cex dex
            cex_dex_bundle_count:                 Self::total_count_by_type(
                |b| b == MevType::CexDexQuotes,
                bundles,
            ),
            cex_dex_fund_count:                   Self::unique_funds(
                |b| b == MevType::CexDexQuotes,
                bundles,
            ),
            cex_dex_total_profit:                 Self::total_profit_by_type(
                |f| f ==MevType::CexDexQuotes,
                bundles,
            ),
            cex_dex_total_revenue:                Self::total_revenue_by_type(
                |f| f ==MevType::CexDexQuotes,
                bundles,
            ),
            cex_dex_average_profit_margin:        Self::average_profit_margin(
                |f| f ==MevType::CexDexQuotes,
                bundles,
            )
            .unwrap_or_default(),
            cex_dex_top_searcher_profit:          cex_dex_searcher_prof_addr,
            cex_dex_top_searcher_revenue:         cex_dex_searcher_rev_addr,
            cex_dex_top_searcher_profit_amt:      cex_dex_searcher_prof,
            cex_dex_top_searcher_revenue_amt:     cex_dex_searcher_rev,
            cex_dex_top_fund_profit_amt:          cex_dex_fund_profit,
            cex_dex_top_fund_profit:              cex_dex_fund_profit_addr,
            cex_dex_top_fund_revenue:             cex_dex_fund_rev_addr,
            cex_dex_top_fund_revenue_amt:         cex_dex_fund_rev,
            cex_dex_most_arbed_dex_profit_amt:    cex_dex_dex_prof,
            cex_dex_most_arbed_dex_profit:        cex_dex_dex_addr_prof,
            cex_dex_most_arbed_dex_revenue:       cex_dex_dex_addr_rev,
            cex_dex_most_arbed_dex_revenue_amt:   cex_dex_dex_rev,
            cex_dex_most_arbed_pair_profit_amt:   Some(cex_dex_pair_prof),
            cex_dex_most_arbed_pair_profit:       cex_dex_pair_addr_prof,
            cex_dex_most_arbed_pair_revenue:      cex_dex_pair_addr_rev,
            cex_dex_most_arbed_pair_revenue_amt:  Some(cex_dex_pair_rev),
            cex_dex_most_arbed_pool_revenue_amt:  cex_dex_pool_rev,
            cex_dex_most_arbed_pool_profit_amt:   cex_dex_pool_prof,
            cex_dex_most_arbed_pool_profit:       cex_dex_pool_addr_prof,
            cex_dex_most_arbed_pool_revenue:      cex_dex_pool_addr_rev,
            cex_dex_biggest_arb_profit:           cex_dex_biggest_tx_prof,
            cex_dex_biggest_arb_profit_amt:       cex_dex_biggest_prof,
            cex_dex_biggest_arb_revenue:          cex_dex_biggest_tx_rev,
            cex_dex_biggest_arb_revenue_amt:      cex_dex_biggest_rev,
            cex_dex_searcher_eoa_all_profit:      cex_dex_all_searcher_prof_addr_eoa,
            cex_dex_searcher_eoa_all_profit_amt:  cex_dex_all_searcher_prof_eoa,
            cex_dex_searcher_eoa_all_revenue:     cex_dex_all_searcher_rev_addr_eoa,
            cex_dex_searcher_eoa_all_revenue_amt: cex_dex_all_searcher_rev_eoa,
            cex_dex_searcher_eoa_count:           Self::unique_eoa(
                |b| b == MevType::CexDexQuotes,
                bundles,
            ),
            cex_dex_mev_contract_all_profit:      cex_dex_all_searcher_prof_addr_contract,
            cex_dex_mev_contract_all_profit_amt:  cex_dex_all_searcher_prof_contract,
            cex_dex_mev_contract_all_revenue:     cex_dex_all_searcher_rev_addr_contract,
            cex_dex_mev_contract_all_revenue_amt: cex_dex_all_searcher_rev_contract,
            cex_dex_mev_contract_count:           Self::unique_contract(
                |b| b == MevType::CexDexQuotes,
                bundles,
            ),
            cex_dex_fund_all_profit:              cex_dex_all_funds_profit_addr,
            cex_dex_fund_all_profit_amt:          cex_dex_all_funds_profit,
            cex_dex_fund_all_revenue:             cex_dex_all_funds_rev_addr,
            cex_dex_fund_all_revenue_amt:         cex_dex_all_funds_rev,
            cex_dex_arbed_dex_all_profit:         cex_dex_all_dexes_addr_prof,
            cex_dex_arbed_dex_all_profit_amt:     cex_dex_all_dexes_prof,
            cex_dex_arbed_dex_all_revenue:        cex_dex_all_dexes_addr_rev,
            cex_dex_arbed_dex_all_revenue_amt:    cex_dex_all_dexes_rev,
            cex_dex_arbed_pair_all_profit:        cex_dex_all_pairs_addr_prof,
            cex_dex_arbed_pair_all_profit_amt:    cex_dex_all_pairs_prof,
            cex_dex_arbed_pair_all_revenue:       cex_dex_all_pairs_addr_rev,
            cex_dex_arbed_pair_all_revenue_amt:   cex_dex_all_pairs_rev,
            cex_dex_arbed_pool_all_profit:        cex_dex_all_pools_addr_prof,
            cex_dex_arbed_pool_all_profit_amt:    cex_dex_all_pools_prof,
            cex_dex_arbed_pool_all_revenue:       cex_dex_all_pools_addr_rev,
            cex_dex_arbed_pool_all_revenue_amt:   cex_dex_all_pools_rev,

            // liquidation
            liquidation_bundle_count:             Self::total_count_by_type(
                |b| b == MevType::Liquidation,
                bundles,
            ),
            liquidation_top_searcher_profit:      liquidation_searcher_prof_addr,
            liquidation_top_searcher_revenue:     liquidation_searcher_rev_addr,
            liquidation_top_searcher_profit_amt:  liquidation_searcher_prof,
            liquidation_top_searcher_revenue_amt: liquidation_searcher_rev,
            liquidation_average_profit_margin:    Self::average_profit_margin(
                |b| b == MevType::Liquidation,
                bundles,
            )
            .unwrap_or_default(),
            liquidation_total_revenue:            Self::total_revenue_by_type(
                |b| b == MevType::Liquidation,
                bundles,
            ),

            liquidation_total_profit:                 Self::total_profit_by_type(
                |b| b == MevType::Liquidation,
                bundles,
            ),
            most_liquidated_token_revenue_amt:        liq_most_rev,
            most_liquidated_token_profit_amt:         liq_most_prof,
            most_liquidated_token_revenue:            liq_most_token_rev.unwrap_or_default(),
            most_liquidated_token_profit:             liq_most_token_prof.unwrap_or_default(),
            total_usd_liquidated:                     Self::total_revenue_by_type(
                |b| b == MevType::Liquidation,
                bundles,
            ),
            liquidation_fund_count:                   Self::unique_funds(
                |b| b == MevType::Liquidation,
                bundles,
            ),
            liquidation_top_fund_profit_amt:          liquidation_fund_profit,
            liquidation_top_fund_profit:              liquidation_fund_profit_addr,
            liquidation_top_fund_revenue:             liquidation_fund_rev_addr,
            liquidation_top_fund_revenue_amt:         liquidation_fund_rev,
            liquidation_searcher_eoa_all_profit:      liquidation_all_searcher_prof_addr_eoa,
            liquidation_searcher_eoa_all_profit_amt:  liquidation_all_searcher_prof_eoa,
            liquidation_searcher_eoa_all_revenue:     liquidation_all_searcher_rev_addr_eoa,
            liquidation_searcher_eoa_all_revenue_amt: liquidation_all_searcher_rev_eoa,
            liquidation_searcher_eoa_count:           Self::unique_eoa(
                |b| b == MevType::Liquidation,
                bundles,
            ),
            liquidation_mev_contract_all_profit:      liquidation_all_searcher_prof_addr_contract,
            liquidation_mev_contract_all_profit_amt:  liquidation_all_searcher_prof_contract,
            liquidation_mev_contract_all_revenue:     liquidation_all_searcher_rev_addr_contract,
            liquidation_mev_contract_all_revenue_amt: liquidation_all_searcher_rev_contract,
            liquidation_mev_contract_count:           Self::unique_contract(
                |b| b == MevType::Liquidation,
                bundles,
            ),
            liquidation_fund_all_profit:              liquidation_all_funds_profit_addr,
            liquidation_fund_all_profit_amt:          liquidation_all_funds_profit,
            liquidation_fund_all_revenue:             liquidation_all_funds_rev_addr,
            liquidation_fund_all_revenue_amt:         liquidation_all_funds_rev,
            liquidated_tokens_profit:                 liq_all_token_prof,
            liquidated_tokens_profit_amt:             liq_all_prof,
            liquidated_tokens_revenue:                liq_all_token_rev,
            liquidated_tokens_revenue_amt:            liq_all_rev,
            liquidated_biggest_arb_profit:            liquidation_biggest_tx_prof,
            liquidated_biggest_arb_profit_amt:        liquidation_biggest_prof,
            liquidated_biggest_arb_revenue:           liquidation_biggest_tx_rev,
            liquidated_biggest_arb_revenue_amt:       liquidation_biggest_rev,

            builder_profit_usd:     block.builder_profit_usd,
            builder_profit_eth:     block.builder_eth_profit,
            builder_revenue_usd:    block.builder_profit_usd
                + block.proposer_profit_usd.unwrap_or(0.0),
            builder_revenue_eth:    block.builder_eth_profit
                + (block.proposer_profit_usd.unwrap_or(0.0) / block.eth_price),
            builder_mev_profit_usd: block.builder_mev_profit_usd,
            builder_mev_profit_eth: block.builder_mev_profit_usd / block.eth_price,
            builder_name:           block.builder_name.clone(),
            builder_address:        block.builder_address,
            proposer_profit_usd:    block.proposer_profit_usd,
            proposer_profit_eth:    block.proposer_profit_usd.map(|p| p / block.eth_price),
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
            BundleData::CexDexQuote(c) => c.swaps.iter().map(|s| s.pool).collect::<Vec<_>>(),
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
            BundleData::CexDexQuote(c) => c.swaps.iter().map(|s| s.protocol).collect::<Vec<_>>(),
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

    fn get_pair_fn(data: &BundleData) -> Vec<TokenPairDetails> {
        match data {
            BundleData::Jit(j) => j
                .victim_swaps
                .iter()
                .flatten()
                .map(|s| (s.token_in.clone(), s.token_out.clone()).into())
                .collect::<Vec<_>>(),
            BundleData::JitSandwich(j) => j
                .victim_swaps
                .iter()
                .flatten()
                .map(|s| (s.token_in.clone(), s.token_out.clone()).into())
                .collect::<Vec<_>>(),
            BundleData::CexDex(c) => c
                .swaps
                .iter()
                .map(|s| (s.token_in.clone(), s.token_out.clone()).into())
                .collect::<Vec<_>>(),
            BundleData::CexDexQuote(c) => c
                .swaps
                .iter()
                .map(|s| (s.token_in.clone(), s.token_out.clone()).into())
                .collect::<Vec<_>>(),
            BundleData::Sandwich(c) => c
                .victim_swaps
                .iter()
                .flatten()
                .map(|s| (s.token_in.clone(), s.token_out.clone()).into())
                .collect::<Vec<_>>(),
            BundleData::AtomicArb(a) => a
                .swaps
                .iter()
                .map(|s| (s.token_in.clone(), s.token_out.clone()).into())
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

    fn total_count_by_type(mev_type: impl Fn(MevType) -> bool, bundles: &[Bundle]) -> u64 {
        bundles
            .iter()
            .filter(|b| mev_type(b.data.mev_type()))
            .count() as u64
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

    fn all_funds_by_type_profit(
        mev_type: impl Fn(MevType) -> bool,
        bundles: &[Bundle],
    ) -> Vec<(Fund, f64)> {
        let mut map = HashMap::new();
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
            .for_each(|(f, amt)| {
                *map.entry(f).or_insert(0.0) += amt;
            });

        map.into_iter().collect_vec()
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

    fn all_funds_by_type_rev(
        mev_type: impl Fn(MevType) -> bool,
        bundles: &[Bundle],
    ) -> Vec<(Fund, f64)> {
        let mut map = HashMap::new();
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
            .for_each(|(f, amt)| {
                *map.entry(f).or_insert(0.0) += amt;
            });

        map.into_iter().collect_vec()
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

    fn all_searchers_by_profit(
        mev_type: impl Fn(MevType) -> bool,
        bundles: &[Bundle],
        mev_contract: bool,
    ) -> Vec<(Address, f64)> {
        let mut map = HashMap::new();
        bundles
            .iter()
            .filter(|b| mev_type(b.data.mev_type()))
            .for_each(|r| {
                if mev_contract {
                    if let Some(contract) = r.header.mev_contract {
                        *map.entry(contract).or_insert(0.0) += r.header.profit_usd;
                    }
                } else {
                    *map.entry(r.header.eoa).or_insert(0.0) += r.header.profit_usd;
                }
            });

        map.into_iter().collect_vec()
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

    fn all_searchers_by_rev(
        mev_type: impl Fn(MevType) -> bool,
        bundles: &[Bundle],
        mev_contract: bool,
    ) -> Vec<(Address, f64)> {
        let mut map = HashMap::new();
        bundles
            .iter()
            .filter(|b| mev_type(b.data.mev_type()))
            .for_each(|r| {
                if mev_contract {
                    if let Some(contract) = r.header.mev_contract {
                        *map.entry(contract).or_insert(0.0) +=
                            r.header.profit_usd + r.header.bribe_usd;
                    }
                } else {
                    *map.entry(r.header.eoa).or_insert(0.0) +=
                        r.header.profit_usd + r.header.bribe_usd;
                }
            });

        map.into_iter().collect_vec()
    }

    fn most_transacted_pool(
        mev_type: impl Fn(MevType) -> bool,
        bundles: &[Bundle],
        f: impl Fn(&BundleData) -> Vec<Address>,
    ) -> Option<(Address, Address, f64, f64)> {
        Self::most_transacted(mev_type, bundles, f)
    }

    fn all_transacted_pools(
        mev_type: impl Fn(MevType) -> bool,
        bundles: &[Bundle],
        f: impl Fn(&BundleData) -> Vec<Address>,
    ) -> (Vec<(Address, f64)>, Vec<(Address, f64)>) {
        Self::all_transacted(mev_type, bundles, f)
    }

    fn most_transacted_pair(
        mev_type: impl Fn(MevType) -> bool,
        bundles: &[Bundle],
        f: impl Fn(&BundleData) -> Vec<TokenPairDetails>,
    ) -> Option<(TokenPairDetails, TokenPairDetails, f64, f64)> {
        Self::most_transacted(mev_type, bundles, f)
    }

    fn all_transacted_pairs(
        mev_type: impl Fn(MevType) -> bool,
        bundles: &[Bundle],
        f: impl Fn(&BundleData) -> Vec<TokenPairDetails>,
    ) -> (Vec<(TokenPairDetails, f64)>, Vec<(TokenPairDetails, f64)>) {
        Self::all_transacted(mev_type, bundles, f)
    }

    fn most_transacted_dex(
        mev_type: impl Fn(MevType) -> bool,
        bundles: &[Bundle],
        f: impl Fn(&BundleData) -> Vec<Protocol>,
    ) -> Option<(Protocol, Protocol, f64, f64)> {
        Self::most_transacted(mev_type, bundles, f)
    }

    fn all_transacted_dexes(
        mev_type: impl Fn(MevType) -> bool,
        bundles: &[Bundle],
        f: impl Fn(&BundleData) -> Vec<Protocol>,
    ) -> (Vec<(Protocol, f64)>, Vec<(Protocol, f64)>) {
        Self::all_transacted(mev_type, bundles, f)
    }

    fn average_profit_margin(
        mev_type: impl Fn(MevType) -> bool,
        bundles: &[Bundle],
    ) -> Option<f64> {
        Some(
            bundles
                .iter()
                .filter(|b| mev_type(b.data.mev_type()) && b.header.bribe_usd != 0.0)
                .map(|s| s.header.profit_usd / (s.header.profit_usd + s.header.bribe_usd).abs())
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

    fn unique_eoa(mev_type: fn(MevType) -> bool, bundles: &[Bundle]) -> u64 {
        bundles
            .iter()
            .filter(|b| mev_type(b.data.mev_type()))
            .map(|b| b.header.eoa)
            .unique()
            .count() as u64
    }

    fn unique_contract(mev_type: fn(MevType) -> bool, bundles: &[Bundle]) -> u64 {
        bundles
            .iter()
            .filter(|b| mev_type(b.data.mev_type()))
            .filter_map(|b| b.header.mev_contract)
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

    fn most_transacted<Ty: Hash + Eq + Clone>(
        mev_type: impl Fn(MevType) -> bool,
        bundles: &[Bundle],
        f: impl Fn(&BundleData) -> Vec<Ty>,
    ) -> Option<(Ty, Ty, f64, f64)> {
        let (profit_ty, profit_usd) = bundles
            .iter()
            .filter(|b| mev_type(b.data.mev_type()))
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
            .max_by(|a, b| a.1.iter().sum::<f64>().total_cmp(&b.1.iter().sum::<f64>()))
            .map(|t| (t.0.clone(), t.1.iter().sum::<f64>()))?;

        let (rev_ty, rev_usd) = bundles
            .iter()
            .filter(|b| mev_type(b.data.mev_type()))
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
            .max_by(|a, b| a.1.iter().sum::<f64>().total_cmp(&b.1.iter().sum::<f64>()))
            .map(|t| (t.0.clone(), t.1.iter().sum::<f64>()))?;

        Some((profit_ty.clone(), rev_ty.clone(), profit_usd, rev_usd))
    }

    fn all_transacted<Ty: Hash + Eq + Clone>(
        mev_type: impl Fn(MevType) -> bool,
        bundles: &[Bundle],
        f: impl Fn(&BundleData) -> Vec<Ty>,
    ) -> (Vec<(Ty, f64)>, Vec<(Ty, f64)>) {
        let profit = bundles
            .iter()
            .filter(|b| mev_type(b.data.mev_type()))
            .flat_map(|b| {
                let res = f(&b.data);
                let mut merged = Vec::with_capacity(res.len());

                res.into_iter()
                    .for_each(|r| merged.push((r, b.header.profit_usd)));
                merged
            })
            .into_group_map()
            .into_iter()
            .map(|(k, v)| (k, v.iter().sum::<f64>()))
            .collect_vec();

        let revenue = bundles
            .iter()
            .filter(|b| mev_type(b.data.mev_type()))
            .flat_map(|b| {
                let res = f(&b.data);
                let mut merged = Vec::with_capacity(res.len());

                res.into_iter()
                    .for_each(|r| merged.push((r, b.header.profit_usd + b.header.bribe_usd)));
                merged
            })
            .into_group_map()
            .into_iter()
            .map(|(k, v)| (k, v.iter().sum::<f64>()))
            .collect_vec();

        (profit, revenue)
    }
}

pub trait FourOptionUnzip<A, B, C, D> {
    fn four_unzip(self) -> (Option<A>, Option<B>, Option<C>, Option<D>)
    where
        Self: Sized;
}

impl<A, B, C, D> FourOptionUnzip<A, B, C, D> for Option<(A, B, C, D)> {
    fn four_unzip(self) -> (Option<A>, Option<B>, Option<C>, Option<D>)
    where
        Self: Sized,
    {
        self.map(|i| (Some(i.0), Some(i.1), Some(i.2), Some(i.3)))
            .unwrap_or_default()
    }
}

pub trait TupleTwoVecUnzip<A, B, C, D> {
    fn four_unzip(self) -> (Vec<A>, Vec<B>, Vec<C>, Vec<D>)
    where
        Self: Sized;
}

impl<A, B, C, D> TupleTwoVecUnzip<A, B, C, D> for (Vec<(A, B)>, Vec<(C, D)>) {
    fn four_unzip(self) -> (Vec<A>, Vec<B>, Vec<C>, Vec<D>)
    where
        Self: Sized,
    {
        let (a, b) = self.0.into_iter().unzip();
        let (c, d) = self.1.into_iter().unzip();
        (a, b, c, d)
    }
}

#[derive(Default, Debug, Clone, Hash, PartialEq, Eq)]
pub struct TokenPairDetails {
    pub address0: Address,
    pub symbol0: String,
    pub address1: Address,
    pub symbol1: String,
}

impl Serialize for TokenPairDetails {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        (
            (format!("{:?}", self.address0), self.symbol0.clone()),
            (format!("{:?}", self.address1), self.symbol1.clone()),
        )
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TokenPairDetails {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        let ((token0_address, token0_symbol), (token1_address, token1_symbol)): (
            (String, String),
            (String, String),
        ) = serde::Deserialize::deserialize(deserializer)?;

        Ok(Self {
            address0: Address::from_str(&token0_address).unwrap_or_default(),
            symbol0: token0_symbol,
            address1: Address::from_str(&token1_address).unwrap_or_default(),
            symbol1: token1_symbol,
        })
    }
}

impl From<(TokenInfoWithAddress, TokenInfoWithAddress)> for TokenPairDetails {
    fn from(value: (TokenInfoWithAddress, TokenInfoWithAddress)) -> Self {
        let (token0, token1) = if Pair(value.0.address, value.1.address).is_ordered() {
            value
        } else {
            (value.1, value.0)
        };

        Self {
            address0: token0.address,
            symbol0: token0.symbol.clone(),
            address1: token1.address,
            symbol1: token1.symbol.clone(),
        }
    }
}

#[derive(Default, Debug, Clone, Hash, PartialEq, Eq)]
pub struct SingleTokenDetails {
    pub address: Address,
    pub symbol: String,
}

impl Serialize for SingleTokenDetails {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        (format!("{:?}", self.address), self.symbol.clone()).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SingleTokenDetails {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        let (address, symbol): (String, String) = serde::Deserialize::deserialize(deserializer)?;

        Ok(Self { address: Address::from_str(&address).unwrap_or_default(), symbol })
    }
}

impl From<TokenInfoWithAddress> for SingleTokenDetails {
    fn from(value: TokenInfoWithAddress) -> Self {
        Self { address: value.address, symbol: value.inner.symbol }
    }
}

impl Default for BlockAnalysis {
    fn default() -> Self {
        BlockAnalysis {
            atomic_mev_contract_all_profit: vec![Default::default()],
            atomic_mev_contract_all_profit_amt: vec![Default::default()],
            atomic_mev_contract_all_revenue: vec![Default::default()],
            atomic_mev_contract_all_revenue_amt: vec![Default::default()],
            atomic_mev_contract_count: Default::default(),
            atomic_searcher_eoa_all_profit: vec![Default::default()],
            atomic_searcher_eoa_all_profit_amt: vec![Default::default()],
            atomic_searcher_eoa_all_revenue: vec![Default::default()],
            atomic_searcher_eoa_all_revenue_amt: vec![Default::default()],
            atomic_searcher_eoa_count: Default::default(),

            atomic_fund_all_profit: vec![Fund::JaneStreet],
            atomic_fund_all_profit_amt: vec![0.0],
            atomic_fund_all_revenue: vec![Fund::JaneStreet],
            atomic_fund_all_revenue_amt: vec![0.0],

            atomic_arbed_pool_all_profit: vec![Default::default()],
            atomic_arbed_pool_all_profit_amt: vec![0.0],
            atomic_arbed_pool_all_revenue: vec![Default::default()],
            atomic_arbed_pool_all_revenue_amt: vec![0.0],

            atomic_arbed_pair_all_profit: vec![Default::default()],
            atomic_arbed_pair_all_profit_amt: vec![0.0],
            atomic_arbed_pair_all_revenue: vec![Default::default()],
            atomic_arbed_pair_all_revenue_amt: vec![0.0],

            atomic_arbed_dex_all_profit: vec![Protocol::UniswapV2],
            atomic_arbed_dex_all_profit_amt: vec![0.0],
            atomic_arbed_dex_all_revenue: vec![Protocol::UniswapV2],
            atomic_arbed_dex_all_revenue_amt: vec![0.0],

            sandwich_mev_contract_all_profit: vec![Default::default()],
            sandwich_mev_contract_all_profit_amt: vec![Default::default()],
            sandwich_mev_contract_all_revenue: vec![Default::default()],
            sandwich_mev_contract_all_revenue_amt: vec![Default::default()],
            sandwich_mev_contract_count: Default::default(),
            sandwich_searcher_eoa_all_profit: vec![Default::default()],
            sandwich_searcher_eoa_all_profit_amt: vec![Default::default()],
            sandwich_searcher_eoa_all_revenue: vec![Default::default()],
            sandwich_searcher_eoa_all_revenue_amt: vec![Default::default()],
            sandwich_searcher_eoa_count: Default::default(),

            sandwich_fund_all_profit: vec![Fund::JaneStreet],
            sandwich_fund_all_profit_amt: vec![0.0],
            sandwich_fund_all_revenue: vec![Fund::JaneStreet],
            sandwich_fund_all_revenue_amt: vec![0.0],

            sandwich_arbed_pool_all_profit: vec![Default::default()],
            sandwich_arbed_pool_all_profit_amt: vec![0.0],
            sandwich_arbed_pool_all_revenue: vec![Default::default()],
            sandwich_arbed_pool_all_revenue_amt: vec![0.0],

            sandwich_arbed_pair_all_profit: vec![Default::default()],
            sandwich_arbed_pair_all_profit_amt: vec![0.0],
            sandwich_arbed_pair_all_revenue: vec![Default::default()],
            sandwich_arbed_pair_all_revenue_amt: vec![0.0],

            sandwich_arbed_dex_all_profit: vec![Protocol::UniswapV2],
            sandwich_arbed_dex_all_profit_amt: vec![0.0],
            sandwich_arbed_dex_all_revenue: vec![Protocol::UniswapV2],
            sandwich_arbed_dex_all_revenue_amt: vec![0.0],

            jit_mev_contract_all_profit: vec![Default::default()],
            jit_mev_contract_all_profit_amt: vec![Default::default()],
            jit_mev_contract_all_revenue: vec![Default::default()],
            jit_mev_contract_all_revenue_amt: vec![Default::default()],
            jit_mev_contract_count: Default::default(),
            jit_searcher_eoa_all_profit: vec![Default::default()],
            jit_searcher_eoa_all_profit_amt: vec![Default::default()],
            jit_searcher_eoa_all_revenue: vec![Default::default()],
            jit_searcher_eoa_all_revenue_amt: vec![Default::default()],
            jit_searcher_eoa_count: Default::default(),

            jit_fund_all_profit: vec![Fund::JaneStreet],
            jit_fund_all_profit_amt: vec![0.0],
            jit_fund_all_revenue: vec![Fund::JaneStreet],
            jit_fund_all_revenue_amt: vec![0.0],

            jit_arbed_pool_all_profit: vec![Default::default()],
            jit_arbed_pool_all_profit_amt: vec![0.0],
            jit_arbed_pool_all_revenue: vec![Default::default()],
            jit_arbed_pool_all_revenue_amt: vec![0.0],

            jit_arbed_pair_all_profit: vec![Default::default()],
            jit_arbed_pair_all_profit_amt: vec![0.0],
            jit_arbed_pair_all_revenue: vec![Default::default()],
            jit_arbed_pair_all_revenue_amt: vec![0.0],

            jit_arbed_dex_all_profit: vec![Protocol::UniswapV2],
            jit_arbed_dex_all_profit_amt: vec![0.0],
            jit_arbed_dex_all_revenue: vec![Protocol::UniswapV2],
            jit_arbed_dex_all_revenue_amt: vec![0.0],

            jit_sandwich_mev_contract_all_profit: vec![Default::default()],
            jit_sandwich_mev_contract_all_profit_amt: vec![Default::default()],
            jit_sandwich_mev_contract_all_revenue: vec![Default::default()],
            jit_sandwich_mev_contract_all_revenue_amt: vec![Default::default()],
            jit_sandwich_mev_contract_count: Default::default(),
            jit_sandwich_searcher_eoa_all_profit: vec![Default::default()],
            jit_sandwich_searcher_eoa_all_profit_amt: vec![Default::default()],
            jit_sandwich_searcher_eoa_all_revenue: vec![Default::default()],
            jit_sandwich_searcher_eoa_all_revenue_amt: vec![Default::default()],
            jit_sandwich_searcher_eoa_count: Default::default(),

            jit_sandwich_fund_all_profit: vec![Fund::JaneStreet],
            jit_sandwich_fund_all_profit_amt: vec![0.0],
            jit_sandwich_fund_all_revenue: vec![Fund::JaneStreet],
            jit_sandwich_fund_all_revenue_amt: vec![0.0],

            jit_sandwich_arbed_pool_all_profit: vec![Default::default()],
            jit_sandwich_arbed_pool_all_profit_amt: vec![0.0],
            jit_sandwich_arbed_pool_all_revenue: vec![Default::default()],
            jit_sandwich_arbed_pool_all_revenue_amt: vec![0.0],

            jit_sandwich_arbed_pair_all_profit: vec![Default::default()],
            jit_sandwich_arbed_pair_all_profit_amt: vec![0.0],
            jit_sandwich_arbed_pair_all_revenue: vec![Default::default()],
            jit_sandwich_arbed_pair_all_revenue_amt: vec![0.0],

            jit_sandwich_arbed_dex_all_profit: vec![Protocol::UniswapV2],
            jit_sandwich_arbed_dex_all_profit_amt: vec![0.0],
            jit_sandwich_arbed_dex_all_revenue: vec![Protocol::UniswapV2],
            jit_sandwich_arbed_dex_all_revenue_amt: vec![0.0],

            cex_dex_mev_contract_all_profit: vec![Default::default()],
            cex_dex_mev_contract_all_profit_amt: vec![Default::default()],
            cex_dex_mev_contract_all_revenue: vec![Default::default()],
            cex_dex_mev_contract_all_revenue_amt: vec![Default::default()],
            cex_dex_mev_contract_count: Default::default(),
            cex_dex_searcher_eoa_all_profit: vec![Default::default()],
            cex_dex_searcher_eoa_all_profit_amt: vec![Default::default()],
            cex_dex_searcher_eoa_all_revenue: vec![Default::default()],
            cex_dex_searcher_eoa_all_revenue_amt: vec![Default::default()],
            cex_dex_searcher_eoa_count: Default::default(),

            cex_dex_arbed_dex_all_profit: vec![Protocol::UniswapV2],
            cex_dex_arbed_dex_all_profit_amt: vec![Default::default()],
            cex_dex_arbed_dex_all_revenue: vec![Protocol::UniswapV2],
            cex_dex_arbed_dex_all_revenue_amt: vec![Default::default()],

            cex_dex_fund_all_profit: vec![Fund::JaneStreet],
            cex_dex_fund_all_profit_amt: vec![0.0],
            cex_dex_fund_all_revenue: vec![Fund::JaneStreet],
            cex_dex_fund_all_revenue_amt: vec![0.0],

            cex_dex_arbed_pool_all_profit: vec![Default::default()],
            cex_dex_arbed_pool_all_profit_amt: vec![0.0],
            cex_dex_arbed_pool_all_revenue: vec![Default::default()],
            cex_dex_arbed_pool_all_revenue_amt: vec![0.0],

            cex_dex_arbed_pair_all_profit: vec![Default::default()],
            cex_dex_arbed_pair_all_profit_amt: vec![0.0],
            cex_dex_arbed_pair_all_revenue: vec![Default::default()],
            cex_dex_arbed_pair_all_revenue_amt: vec![0.0],

            liquidation_mev_contract_all_profit: vec![Default::default()],
            liquidation_mev_contract_all_profit_amt: vec![Default::default()],
            liquidation_mev_contract_all_revenue: vec![Default::default()],
            liquidation_mev_contract_all_revenue_amt: vec![Default::default()],
            liquidation_mev_contract_count: Default::default(),
            liquidation_searcher_eoa_all_profit: vec![Default::default()],
            liquidation_searcher_eoa_all_profit_amt: vec![Default::default()],
            liquidation_searcher_eoa_all_revenue: vec![Default::default()],
            liquidation_searcher_eoa_all_revenue_amt: vec![Default::default()],
            liquidation_searcher_eoa_count: Default::default(),

            liquidation_fund_all_profit: vec![Fund::JaneStreet],
            liquidation_fund_all_profit_amt: vec![0.0],
            liquidation_fund_all_revenue: vec![Fund::JaneStreet],
            liquidation_fund_all_revenue_amt: vec![0.0],

            liquidated_tokens_profit: vec![Default::default()],
            liquidated_tokens_profit_amt: vec![0.0],
            liquidated_tokens_revenue: vec![Default::default()],
            liquidated_tokens_revenue_amt: vec![0.0],
            block_number: Default::default(),
            all_total_profit: Default::default(),
            all_total_revenue: Default::default(),
            all_average_profit_margin: Default::default(),
            all_top_searcher_profit: Default::default(),
            all_top_searcher_profit_amt: Default::default(),
            all_top_searcher_revenue: Default::default(),
            all_top_searcher_revenue_amt: Default::default(),
            all_searcher_count: Default::default(),
            all_top_fund_profit: Default::default(),
            all_top_fund_profit_amt: Default::default(),
            all_top_fund_revenue: Default::default(),
            all_top_fund_revenue_amt: Default::default(),
            all_fund_count: Default::default(),
            all_most_arbed_pool_profit: Default::default(),
            all_most_arbed_pool_profit_amt: Default::default(),
            all_most_arbed_pool_revenue: Default::default(),
            all_most_arbed_pool_revenue_amt: Default::default(),
            all_most_arbed_pair_profit: Default::default(),
            all_most_arbed_pair_profit_amt: Default::default(),
            all_most_arbed_pair_revenue: Default::default(),
            all_most_arbed_pair_revenue_amt: Default::default(),
            all_most_arbed_dex_profit: Default::default(),
            all_most_arbed_dex_profit_amt: Default::default(),
            all_most_arbed_dex_revenue: Default::default(),
            all_most_arbed_dex_revenue_amt: Default::default(),
            all_biggest_arb_profit: Default::default(),
            all_biggest_arb_profit_amt: Default::default(),
            all_biggest_arb_revenue: Default::default(),
            all_biggest_arb_revenue_amt: Default::default(),
            atomic_total_profit: Default::default(),
            atomic_total_revenue: Default::default(),
            atomic_average_profit_margin: Default::default(),
            atomic_top_searcher_profit: Default::default(),
            atomic_top_searcher_profit_amt: Default::default(),
            atomic_top_searcher_revenue: Default::default(),
            atomic_top_searcher_revenue_amt: Default::default(),

            atomic_top_fund_profit: Default::default(),
            atomic_top_fund_profit_amt: Default::default(),
            atomic_top_fund_revenue: Default::default(),
            atomic_top_fund_revenue_amt: Default::default(),
            atomic_fund_count: Default::default(),
            atomic_most_arbed_pool_profit: Default::default(),
            atomic_most_arbed_pool_profit_amt: Default::default(),
            atomic_most_arbed_pool_revenue: Default::default(),
            atomic_most_arbed_pool_revenue_amt: Default::default(),
            atomic_most_arbed_pair_profit: Default::default(),
            atomic_most_arbed_pair_profit_amt: Default::default(),
            atomic_most_arbed_pair_revenue: Default::default(),
            atomic_most_arbed_pair_revenue_amt: Default::default(),
            atomic_most_arbed_dex_profit: Default::default(),
            atomic_most_arbed_dex_profit_amt: Default::default(),
            atomic_most_arbed_dex_revenue: Default::default(),
            atomic_most_arbed_dex_revenue_amt: Default::default(),
            atomic_biggest_arb_profit: Default::default(),
            atomic_biggest_arb_profit_amt: Default::default(),
            atomic_biggest_arb_revenue: Default::default(),
            atomic_biggest_arb_revenue_amt: Default::default(),
            sandwich_total_profit: Default::default(),
            sandwich_total_revenue: Default::default(),
            sandwich_average_profit_margin: Default::default(),
            sandwich_top_searcher_profit: Default::default(),
            sandwich_top_searcher_profit_amt: Default::default(),
            sandwich_top_searcher_revenue: Default::default(),
            sandwich_top_searcher_revenue_amt: Default::default(),
            sandwich_top_fund_profit: Default::default(),
            sandwich_top_fund_profit_amt: Default::default(),
            sandwich_top_fund_revenue: Default::default(),
            sandwich_top_fund_revenue_amt: Default::default(),
            sandwich_fund_count: Default::default(),
            sandwich_most_arbed_pool_profit: Default::default(),
            sandwich_most_arbed_pool_profit_amt: Default::default(),
            sandwich_most_arbed_pool_revenue: Default::default(),
            sandwich_most_arbed_pool_revenue_amt: Default::default(),
            sandwich_most_arbed_pair_profit: Default::default(),
            sandwich_most_arbed_pair_profit_amt: Default::default(),
            sandwich_most_arbed_pair_revenue: Default::default(),
            sandwich_most_arbed_pair_revenue_amt: Default::default(),
            sandwich_most_arbed_dex_profit: Default::default(),
            sandwich_most_arbed_dex_profit_amt: Default::default(),
            sandwich_most_arbed_dex_revenue: Default::default(),
            sandwich_most_arbed_dex_revenue_amt: Default::default(),
            sandwich_biggest_arb_profit: Default::default(),
            sandwich_biggest_arb_profit_amt: Default::default(),
            sandwich_biggest_arb_revenue: Default::default(),
            sandwich_biggest_arb_revenue_amt: Default::default(),
            jit_total_profit: Default::default(),
            jit_total_revenue: Default::default(),
            jit_average_profit_margin: Default::default(),
            jit_top_searcher_profit: Default::default(),
            jit_top_searcher_profit_amt: Default::default(),
            jit_top_searcher_revenue: Default::default(),
            jit_top_searcher_revenue_amt: Default::default(),
            jit_top_fund_profit: Default::default(),
            jit_top_fund_profit_amt: Default::default(),
            jit_top_fund_revenue: Default::default(),
            jit_top_fund_revenue_amt: Default::default(),
            jit_fund_count: Default::default(),
            jit_most_arbed_pool_profit: Default::default(),
            jit_most_arbed_pool_profit_amt: Default::default(),
            jit_most_arbed_pool_revenue: Default::default(),
            jit_most_arbed_pool_revenue_amt: Default::default(),
            jit_most_arbed_pair_profit: Default::default(),
            jit_most_arbed_pair_profit_amt: Default::default(),
            jit_most_arbed_pair_revenue: Default::default(),
            jit_most_arbed_pair_revenue_amt: Default::default(),
            jit_most_arbed_dex_profit: Default::default(),
            jit_most_arbed_dex_profit_amt: Default::default(),
            jit_most_arbed_dex_revenue: Default::default(),
            jit_most_arbed_dex_revenue_amt: Default::default(),
            jit_biggest_arb_profit: Default::default(),
            jit_biggest_arb_profit_amt: Default::default(),
            jit_biggest_arb_revenue: Default::default(),
            jit_biggest_arb_revenue_amt: Default::default(),
            jit_sandwich_total_profit: Default::default(),
            jit_sandwich_total_revenue: Default::default(),
            jit_sandwich_average_profit_margin: Default::default(),
            jit_sandwich_top_searcher_profit: Default::default(),
            jit_sandwich_top_searcher_profit_amt: Default::default(),
            jit_sandwich_top_searcher_revenue: Default::default(),
            jit_sandwich_top_searcher_revenue_amt: Default::default(),
            jit_sandwich_top_fund_profit: Default::default(),
            jit_sandwich_top_fund_profit_amt: Default::default(),
            jit_sandwich_top_fund_revenue: Default::default(),
            jit_sandwich_top_fund_revenue_amt: Default::default(),
            jit_sandwich_fund_count: Default::default(),
            jit_sandwich_most_arbed_pool_profit: Default::default(),
            jit_sandwich_most_arbed_pool_profit_amt: Default::default(),
            jit_sandwich_most_arbed_pool_revenue: Default::default(),
            jit_sandwich_most_arbed_pool_revenue_amt: Default::default(),
            jit_sandwich_most_arbed_pair_profit: Default::default(),
            jit_sandwich_most_arbed_pair_profit_amt: Default::default(),
            jit_sandwich_most_arbed_pair_revenue: Default::default(),
            jit_sandwich_most_arbed_pair_revenue_amt: Default::default(),
            jit_sandwich_most_arbed_dex_profit: Default::default(),
            jit_sandwich_most_arbed_dex_profit_amt: Default::default(),
            jit_sandwich_most_arbed_dex_revenue: Default::default(),
            jit_sandwich_most_arbed_dex_revenue_amt: Default::default(),
            jit_sandwich_biggest_arb_profit: Default::default(),
            jit_sandwich_biggest_arb_profit_amt: Default::default(),
            jit_sandwich_biggest_arb_revenue: Default::default(),
            jit_sandwich_biggest_arb_revenue_amt: Default::default(),
            cex_dex_total_profit: Default::default(),
            cex_dex_total_revenue: Default::default(),
            cex_dex_average_profit_margin: Default::default(),
            cex_dex_top_searcher_profit: Default::default(),
            cex_dex_top_searcher_profit_amt: Default::default(),
            cex_dex_top_searcher_revenue: Default::default(),
            cex_dex_top_searcher_revenue_amt: Default::default(),
            cex_dex_top_fund_profit: Default::default(),
            cex_dex_top_fund_profit_amt: Default::default(),
            cex_dex_top_fund_revenue: Default::default(),
            cex_dex_top_fund_revenue_amt: Default::default(),
            cex_dex_fund_count: Default::default(),
            cex_dex_most_arbed_pool_profit: Default::default(),
            cex_dex_most_arbed_pool_profit_amt: Default::default(),
            cex_dex_most_arbed_pool_revenue: Default::default(),
            cex_dex_most_arbed_pool_revenue_amt: Default::default(),
            cex_dex_most_arbed_pair_profit: Default::default(),
            cex_dex_most_arbed_pair_profit_amt: Default::default(),
            cex_dex_most_arbed_pair_revenue: Default::default(),
            cex_dex_most_arbed_pair_revenue_amt: Default::default(),
            cex_dex_most_arbed_dex_profit: Default::default(),
            cex_dex_most_arbed_dex_profit_amt: Default::default(),
            cex_dex_most_arbed_dex_revenue: Default::default(),
            cex_dex_most_arbed_dex_revenue_amt: Default::default(),
            cex_dex_bundle_count: Default::default(),
            all_bundle_count: Default::default(),
            atomic_bundle_count: Default::default(),
            jit_bundle_count: Default::default(),
            sandwich_bundle_count: Default::default(),
            liquidation_bundle_count: Default::default(),
            jit_sandwich_bundle_count: Default::default(),
            cex_dex_biggest_arb_profit: Default::default(),
            cex_dex_biggest_arb_profit_amt: Default::default(),
            cex_dex_biggest_arb_revenue: Default::default(),
            cex_dex_biggest_arb_revenue_amt: Default::default(),
            liquidation_total_profit: Default::default(),
            liquidation_total_revenue: Default::default(),
            liquidation_average_profit_margin: Default::default(),
            liquidation_top_searcher_profit: Default::default(),
            liquidation_top_searcher_profit_amt: Default::default(),
            liquidation_top_searcher_revenue: Default::default(),
            liquidation_top_searcher_revenue_amt: Default::default(),
            liquidation_top_fund_profit: Default::default(),
            liquidation_top_fund_profit_amt: Default::default(),
            liquidation_top_fund_revenue: Default::default(),
            liquidation_top_fund_revenue_amt: Default::default(),
            liquidation_fund_count: Default::default(),
            most_liquidated_token_revenue: Default::default(),
            most_liquidated_token_revenue_amt: Default::default(),
            most_liquidated_token_profit: Default::default(),
            most_liquidated_token_profit_amt: Default::default(),
            liquidated_biggest_arb_profit: Default::default(),
            liquidated_biggest_arb_profit_amt: Default::default(),
            liquidated_biggest_arb_revenue: Default::default(),
            liquidated_biggest_arb_revenue_amt: Default::default(),
            total_usd_liquidated: Default::default(),

            builder_address: Default::default(),
            builder_mev_profit_eth: Default::default(),
            builder_mev_profit_usd: Default::default(),
            builder_name: Default::default(),
            builder_profit_eth: Default::default(),
            builder_profit_usd: Default::default(),
            builder_revenue_eth: Default::default(),
            builder_revenue_usd: Default::default(),
            proposer_profit_eth: Default::default(),
            proposer_profit_usd: Default::default(),

            eth_price: Default::default(),
        }
    }
}
