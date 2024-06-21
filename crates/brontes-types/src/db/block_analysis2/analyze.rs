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

pub trait AnalyzeBlock {
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
    ) -> Vec<(Address, f64)> {
        let mut map = HashMap::new();
        bundles
            .iter()
            .filter(|b| mev_type(b.data.mev_type()))
            .for_each(|r| {
                *map.entry(r.header.eoa).or_insert(0.0) += r.header.profit_usd;

                if let Some(contract) = r.header.mev_contract {
                    *map.entry(contract).or_insert(0.0) += r.header.profit_usd;
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
    ) -> Vec<(Address, f64)> {
        let mut map = HashMap::new();
        bundles
            .iter()
            .filter(|b| mev_type(b.data.mev_type()))
            .for_each(|r| {
                *map.entry(r.header.eoa).or_insert(0.0) += r.header.profit_usd + r.header.bribe_usd;

                if let Some(contract) = r.header.mev_contract {
                    *map.entry(contract).or_insert(0.0) += r.header.profit_usd + r.header.bribe_usd;
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
                .map(|s| s.header.profit_usd / s.header.bribe_usd.abs())
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
