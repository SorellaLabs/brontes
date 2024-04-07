use std::{collections::hash_map::Entry, hash::Hash, iter, sync::Arc};

use brontes_database::libmdbx::LibmdbxReader;
use brontes_types::{
    db::dex::PriceAt,
    mev::{Bundle, BundleData, MevType, Sandwich},
    normalized_actions::{
        accounting::ActionAccounting, Actions, NormalizedAggregator, NormalizedSwap,
        NormalizedTransfer,
    },
    tree::{BlockTree, GasDetails, TxInfo},
    ActionIter, FastHashMap, FastHashSet, IntoZipTree, ToFloatNearest, TreeBase, TreeCollector,
    TreeIter, TreeSearchBuilder, UnzipPadded,
};
use itertools::Itertools;
use malachite::{num::basic::traits::Zero, Rational};
use reth_primitives::{Address, B256};

use crate::{shared_utils::SharedInspectorUtils, Inspector, Metadata};

pub struct SandwichInspector<'db, DB: LibmdbxReader> {
    utils: SharedInspectorUtils<'db, DB>,
}

impl<'db, DB: LibmdbxReader> SandwichInspector<'db, DB> {
    pub fn new(quote: Address, db: &'db DB) -> Self {
        Self { utils: SharedInspectorUtils::new(quote, db) }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct PossibleSandwich {
    eoa:                   Address,
    possible_frontruns:    Vec<B256>,
    possible_backrun:      B256,
    mev_executor_contract: Address,
    // mapping of possible frontruns to set of possible victims
    // By definition the victims of latter txes are victims of the former
    victims:               Vec<Vec<B256>>,
}

// Add support for this, where there is a frontrun & then backrun & in between
// there is an unrelated tx that is not frontrun but is backrun. See the rari
// trade here. https://libmev.com/blocks/18215838
impl<DB: LibmdbxReader> Inspector for SandwichInspector<'_, DB> {
    type Result = Vec<Bundle>;

    fn get_id(&self) -> &str {
        "Sandwich"
    }

    fn process_tree(&self, tree: Arc<BlockTree<Actions>>, metadata: Arc<Metadata>) -> Self::Result {
        let search_args = TreeSearchBuilder::default().with_actions([
            Actions::is_swap,
            Actions::is_transfer,
            Actions::is_eth_transfer,
            Actions::is_aggregator,
        ]);

        Self::get_possible_sandwich(tree.clone())
            .into_iter()
            .filter_map(
                |PossibleSandwich {
                     eoa: _,
                     possible_frontruns,
                     possible_backrun,
                     mev_executor_contract,
                     victims,
                 }| {
                    if victims.iter().flatten().count() == 0 {
                        return None
                    };

                    let (victim_swaps_transfers, victim_info): (Vec<_>, Vec<_>) = victims
                        .into_iter()
                        .map(|victim| {
                            (
                                tree.clone()
                                    .collect_txes(&victim, search_args.clone())
                                    .t_map(|a| {
                                        a.into_iter().flatten_specified(
                                            Actions::try_aggregator_ref,
                                            |actions: NormalizedAggregator| {
                                                actions
                                                    .child_actions
                                                    .into_iter()
                                                    .filter(|f| f.is_swap() || f.is_transfer())
                                                    .collect::<Vec<_>>()
                                            },
                                        )
                                    }),
                                victim,
                            )
                        })
                        .try_fold(vec![], |mut acc, (victim_set, hashes)| {
                            let tree = victim_set.tree();
                            let (actions, info) = victim_set
                                .map(|s| {
                                    s.into_iter().split_actions::<(Vec<_>, Vec<_>), _>((
                                        Actions::try_swaps_merged,
                                        Actions::try_transfer,
                                    ))
                                })
                                .into_zip_tree(tree)
                                .tree_zip_with(hashes.into_iter())
                                .t_full_filter_map(|(tree, rest)| {
                                    let (swap, hashes): (Vec<_>, Vec<_>) =
                                        UnzipPadded::unzip_padded(rest);

                                    if !hashes
                                        .iter()
                                        .map(|v| {
                                            let tree = &(*tree.clone());
                                            let d = tree.get_root(*v).unwrap().get_root_action();

                                            d.is_revert()
                                                || mev_executor_contract == d.get_to_address()
                                        })
                                        .any(|d| d)
                                    {
                                        Some((
                                            swap,
                                            hashes
                                                .into_iter()
                                                .map(|hash| {
                                                    tree.get_tx_info(hash, self.utils.db).unwrap()
                                                })
                                                .collect::<Vec<_>>(),
                                        ))
                                    } else {
                                        None
                                    }
                                })?;

                            if actions.is_empty() {
                                None
                            } else {
                                acc.push((actions, info));
                                Some(acc)
                            }
                        })?
                        .into_iter()
                        .unzip();

                    let frontrun_info = possible_frontruns
                        .iter()
                        .flat_map(|pf| tree.get_tx_info(*pf, self.utils.db))
                        .collect::<Vec<_>>();

                    let back_run_info = tree.get_tx_info(possible_backrun, self.utils.db)?;

                    let searcher_actions: Vec<Vec<Actions>> = tree
                        .clone()
                        .collect_txes(
                            possible_frontruns
                                .iter()
                                .copied()
                                .chain(std::iter::once(possible_backrun))
                                .collect::<Vec<_>>()
                                .as_slice(),
                            search_args.clone(),
                        )
                        .collect::<Vec<_>>();

                    self.calculate_sandwich(
                        tree.clone(),
                        metadata.clone(),
                        frontrun_info,
                        back_run_info,
                        searcher_actions,
                        victim_info,
                        victim_swaps_transfers,
                    )
                },
            )
            .collect::<Vec<_>>()
    }
}

impl<DB: LibmdbxReader> SandwichInspector<'_, DB> {
    fn calculate_sandwich(
        &self,
        tree: Arc<BlockTree<Actions>>,
        metadata: Arc<Metadata>,
        mut possible_front_runs_info: Vec<TxInfo>,
        backrun_info: TxInfo,
        mut searcher_actions: Vec<Vec<Actions>>,
        mut victim_info: Vec<Vec<TxInfo>>,
        mut victim_actions: Vec<Vec<(Vec<NormalizedSwap>, Vec<NormalizedTransfer>)>>,
    ) -> Option<Bundle> {
        let back_run_actions = searcher_actions.pop()?;
        let back_run_swaps = back_run_actions
            .clone()
            .into_iter()
            .collect_action_vec(Actions::try_swaps_merged);

        let front_run_swaps = searcher_actions
            .clone()
            .into_iter()
            .map(|action| {
                action
                    .into_iter()
                    .collect_action_vec(Actions::try_swaps_merged)
            })
            .collect::<Vec<_>>();

        //TODO: Check later if this method correctly identifies an incorrect middle
        // front run that is unrelated
        if !Self::has_pool_overlap(&front_run_swaps, &back_run_swaps, &victim_actions, &victim_info)
        {
            // if we don't satisfy a sandwich but we have more than 1 possible front run
            // tx remaining, lets remove the false positive backrun tx and try again
            if possible_front_runs_info.len() > 1 {
                // remove dropped sandwiches
                victim_info.pop()?;
                victim_actions.pop()?;
                let back_run_info = possible_front_runs_info.pop()?;

                if victim_actions
                    .iter()
                    .flatten()
                    .filter_map(
                        |(s, t)| {
                            if s.is_empty() && t.is_empty() {
                                None
                            } else {
                                Some(true)
                            }
                        },
                    )
                    .count()
                    == 0
                {
                    return None
                }

                return self.calculate_sandwich(
                    tree.clone(),
                    metadata.clone(),
                    possible_front_runs_info,
                    back_run_info,
                    searcher_actions,
                    victim_info,
                    victim_actions,
                )
            }

            return None
        }

        let victim_swaps = victim_actions.into_iter().flatten().collect::<Vec<_>>();

        let (frontrun_tx_hash, frontrun_gas_details): (Vec<_>, Vec<_>) = possible_front_runs_info
            .clone()
            .into_iter()
            .map(|info| info.split_to_storage_info())
            .unzip();

        let (victim_swaps_tx_hashes, victim_swaps_gas_details): (Vec<_>, Vec<_>) = victim_info
            .clone()
            .into_iter()
            .map(|info| {
                info.into_iter()
                    .map(|info| info.split_to_storage_info())
                    .unzip::<B256, GasDetails, Vec<B256>, Vec<GasDetails>>()
            })
            .unzip();

        let gas_used = frontrun_gas_details
            .iter()
            .chain([backrun_info.gas_details].iter())
            .map(|g| g.gas_paid())
            .sum::<u128>();

        let gas_used = metadata.get_gas_price_usd(gas_used, self.utils.quote);

        let searcher_deltas = searcher_actions
            .into_iter()
            .flatten()
            .chain(back_run_actions)
            .filter(|f| f.is_transfer() || f.is_eth_transfer())
            .account_for_actions();

        let mev_addresses: FastHashSet<Address> = possible_front_runs_info
            .iter()
            .chain(iter::once(&backrun_info))
            .flat_map(|tx_info| iter::once(&tx_info.eoa).chain(tx_info.mev_contract.as_ref()))
            .cloned()
            .collect();

        let (rev, has_dex_price) = if let Some(rev) = self.utils.get_deltas_usd(
            backrun_info.tx_index,
            PriceAt::After,
            &mev_addresses,
            &searcher_deltas,
            metadata.clone(),
        ) {
            (Some(rev), true)
        } else {
            (Some(Rational::ZERO), false)
        };

        let profit_usd = rev
            .map(|rev| rev - &gas_used)
            .filter(|_| has_dex_price)
            .unwrap_or_default();

        let gas_details: Vec<_> = possible_front_runs_info
            .iter()
            .chain(std::iter::once(&backrun_info))
            .map(|info| info.gas_details)
            .collect();

        let mut bundle_hashes = Vec::new();

        for (index, frontrun_hash) in frontrun_tx_hash.iter().enumerate() {
            bundle_hashes.push(*frontrun_hash);
            if let Some(victim_hashes) = victim_swaps_tx_hashes.get(index) {
                bundle_hashes.extend_from_slice(victim_hashes);
            }
        }
        bundle_hashes.push(backrun_info.tx_hash);

        let header = self.utils.build_bundle_header(
            vec![searcher_deltas],
            bundle_hashes,
            &backrun_info,
            profit_usd.to_float(),
            PriceAt::After,
            &gas_details,
            metadata,
            MevType::Sandwich,
            !has_dex_price,
        );
        tracing::debug!("{:#?}", header.dex_quotes);

        let victim_swaps = victim_swaps
            .into_iter()
            .map(|(swaps, _)| swaps)
            .collect_vec();

        let sandwich = Sandwich {
            frontrun_tx_hash,
            frontrun_gas_details,
            frontrun_swaps: front_run_swaps,
            victim_swaps_tx_hashes,
            victim_swaps_gas_details: victim_swaps_gas_details.into_iter().flatten().collect(),
            victim_swaps,
            backrun_tx_hash: backrun_info.tx_hash,
            backrun_swaps: back_run_swaps,
            backrun_gas_details: backrun_info.gas_details,
        };

        Some(Bundle { header, data: BundleData::Sandwich(sandwich) })
    }

    fn has_pool_overlap(
        front_run_swaps: &[Vec<NormalizedSwap>],
        back_run_swaps: &[NormalizedSwap],
        victim_actions: &[Vec<(Vec<NormalizedSwap>, Vec<NormalizedTransfer>)>],
        victim_info: &[Vec<TxInfo>],
    ) -> bool {
        let f_swap_len = front_run_swaps.len();
        for (i, (chunk_victim_actions, chunk_victim_info)) in
            victim_actions.iter().zip(victim_info).enumerate()
        {
            let chunk_front_run_swaps = &front_run_swaps[0..=i];

            let chunk_back_run_swaps = if f_swap_len > i + 1 {
                let mut res = vec![];
                res.extend(front_run_swaps[i + 1..].iter().flatten().cloned());
                res.extend(back_run_swaps.to_vec().clone());
                res
            } else {
                back_run_swaps.to_vec()
            };

            let front_run_pools = chunk_front_run_swaps
                .iter()
                .flatten()
                .map(|s| s.pool)
                .collect::<FastHashSet<_>>();

            let back_run_pools = chunk_back_run_swaps
                .iter()
                .map(|swap| swap.pool)
                .collect::<FastHashSet<_>>();

            let front_run_tokens = chunk_front_run_swaps
                .iter()
                .flatten()
                .flat_map(|s| {
                    [(s.token_in.address, s.pool, true), (s.token_out.address, s.pool, false)]
                })
                .collect::<FastHashSet<_>>();

            let back_run_tokens = chunk_back_run_swaps
                .iter()
                .flat_map(|s| {
                    [(s.token_in.address, s.pool, true), (s.token_out.address, s.pool, false)]
                })
                .collect::<FastHashSet<_>>();

            // we group all victims by eoa, such that instead of a tx needing to be a
            // victim, a eoa needs to be a victim. this allows for more complex
            // detection such as having a approve and then a swap in different
            // transactions.
            let grouped_victims = itertools::Itertools::into_group_map(
                chunk_victim_info
                    .iter()
                    .zip(chunk_victim_actions)
                    .map(|(info, actions)| (info.eoa, actions)),
            );
            let amount = grouped_victims.len();

            if amount == 0 {
                return false
            }

            // for each victim eoa, ensure they are a victim of a frontrun and a backrun
            // either through a pool or overlapping tokens
            let was_victims: usize = grouped_victims
                .into_values()
                .map(|v| {
                    v.iter()
                        .cloned()
                        .filter(|(a, b)| !(a.is_empty() && b.is_empty()))
                        .any(|(swaps, transfers)| {
                            swaps.iter().any(|s| front_run_pools.contains(&s.pool))
                                || transfers.iter().any(|t| {
                                    // victim has a transfer from the pool that was a token in for
                                    // the sandwich
                                    front_run_tokens.contains(&(t.token.address, t.from, true))
                            // victim has a transfer to the pool that was a token out for the
                            // sandwich 
                                || front_run_tokens.contains(&(t.token.address, t.to, false))
                                })
                        })
                        && v.into_iter()
                            .filter(|(a, b)| !(a.is_empty() && b.is_empty()))
                            .any(|(swaps, transfers)| {
                                swaps.iter().any(|s| back_run_pools.contains(&s.pool))
                                    || transfers.iter().any(|t| {
                                        // victim has a transfer from the pool that was a token in
                                        // for the sandwich
                                        back_run_tokens.contains(&(t.token.address, t.from, true))
                            // victim has a transfer to the pool that was a token out for the
                            // sandwich 
                                || back_run_tokens.contains(&(t.token.address, t.to, false))
                                    })
                            })
                })
                .map(|was_victim| was_victim as usize)
                .sum();

            // if we had more than 50% victims, then we say this was valid. This
            // wiggle room is to deal with unkowns
            if (was_victims as f64) / (amount as f64) < 0.5 {
                return false
            }
        }

        true
    }

    /// Aggregates potential sandwich attacks from both duplicate senders and
    /// MEV contracts.
    ///
    /// This higher-level function concurrently executes
    /// `get_possible_sandwich_duplicate_senders`
    /// and `get_possible_sandwich_duplicate_contracts` to gather a broad set of
    /// potential sandwich attacks. It aims to cover intricate scenarios,
    /// including multiple frontruns and backruns targeting different victims.
    ///
    /// The results from both functions are combined and deduplicated to form a
    /// comprehensive set of potential sandwich attacks.
    fn get_possible_sandwich(tree: Arc<BlockTree<Actions>>) -> Vec<PossibleSandwich> {
        if tree.tx_roots.len() < 3 {
            return vec![]
        }

        let tree_clone_for_senders = tree.clone();
        let tree_clone_for_contracts = tree.clone();

        // Using Rayon to execute functions in parallel
        let (result_senders, result_contracts) = rayon::join(
            || get_possible_sandwich_duplicate_senders(tree_clone_for_senders),
            || get_possible_sandwich_duplicate_contracts(tree_clone_for_contracts),
        );

        // Combine and deduplicate results
        let combined_results = result_senders.into_iter().chain(result_contracts);
        let unique_results: FastHashSet<_> = combined_results.collect();

        unique_results.into_iter().collect()
    }
}

fn get_possible_sandwich_duplicate_senders(tree: Arc<BlockTree<Actions>>) -> Vec<PossibleSandwich> {
    let mut duplicate_senders: FastHashMap<Address, B256> = FastHashMap::default();
    let mut possible_victims: FastHashMap<B256, Vec<B256>> = FastHashMap::default();
    let mut possible_sandwiches: FastHashMap<Address, PossibleSandwich> = FastHashMap::default();

    for root in tree.tx_roots.iter() {
        if root.get_root_action().is_revert() {
            continue
        }
        match duplicate_senders.entry(root.head.address) {
            // If we have not seen this sender before, we insert the tx hash into the map
            Entry::Vacant(v) => {
                v.insert(root.tx_hash);
                possible_victims.insert(root.tx_hash, vec![]);
            }
            Entry::Occupied(mut o) => {
                // Get's prev tx hash for this sender & replaces it with the current tx hash
                let prev_tx_hash = o.insert(root.tx_hash);
                if let Some(frontrun_victims) = possible_victims.remove(&prev_tx_hash) {
                    match possible_sandwiches.entry(root.head.address) {
                        Entry::Vacant(e) => {
                            e.insert(PossibleSandwich {
                                eoa:                   root.head.address,
                                possible_frontruns:    vec![prev_tx_hash],
                                possible_backrun:      root.tx_hash,
                                mev_executor_contract: root.get_to_address(),
                                victims:               vec![frontrun_victims],
                            });
                        }
                        Entry::Occupied(mut o) => {
                            let sandwich = o.get_mut();
                            sandwich.possible_frontruns.push(prev_tx_hash);
                            sandwich.possible_backrun = root.tx_hash;
                            sandwich.victims.push(frontrun_victims);
                        }
                    }
                }

                // Add current transaction hash to the list of transactions for this sender
                o.insert(root.tx_hash);
                possible_victims.insert(root.tx_hash, vec![]);
            }
        }

        // Now, for each existing entry in possible_victims, we add the current
        // transaction hash as a potential victim, if it is not the same as
        // the key (which represents another transaction hash)
        for (k, v) in possible_victims.iter_mut() {
            if k != &root.tx_hash {
                v.push(root.tx_hash);
            }
        }
    }
    possible_sandwiches.values().cloned().collect()
}

/// This function iterates through the block tree to identify potential
/// sandwiches by looking for a contract that is involved in multiple
/// transactions within a block.
///
/// The approach is aimed at uncovering not just standard sandwich attacks but
/// also complex scenarios like the "Big Mac Sandwich", where a sequence of
/// transactions exploits multiple victims with varying slippage tolerances.
fn get_possible_sandwich_duplicate_contracts(
    tree: Arc<BlockTree<Actions>>,
) -> Vec<PossibleSandwich> {
    let mut duplicate_mev_contracts: FastHashMap<Address, (B256, Address)> = FastHashMap::default();
    let mut possible_victims: FastHashMap<B256, Vec<B256>> = FastHashMap::default();
    let mut possible_sandwiches: FastHashMap<Address, PossibleSandwich> = FastHashMap::default();

    for root in tree.tx_roots.iter() {
        if root.get_root_action().is_revert() {
            continue
        }

        match duplicate_mev_contracts.entry(root.get_to_address()) {
            // If this contract has not been called within this block, we insert the tx hash
            // into the map
            Entry::Vacant(duplicate_mev_contract) => {
                duplicate_mev_contract.insert((root.tx_hash, root.head.address));
                possible_victims.insert(root.tx_hash, vec![]);
            }
            Entry::Occupied(mut o) => {
                // Get's prev tx hash &  for this sender & replaces it with the current tx hash
                let (prev_tx_hash, frontrun_eoa) = o.get_mut();

                if let Some(frontrun_victims) = possible_victims.remove(prev_tx_hash) {
                    match possible_sandwiches.entry(root.get_to_address()) {
                        Entry::Vacant(e) => {
                            e.insert(PossibleSandwich {
                                eoa:                   *frontrun_eoa,
                                possible_frontruns:    vec![*prev_tx_hash],
                                possible_backrun:      root.tx_hash,
                                mev_executor_contract: root.get_to_address(),
                                victims:               vec![frontrun_victims],
                            });
                        }
                        Entry::Occupied(mut o) => {
                            let sandwich = o.get_mut();
                            sandwich.possible_frontruns.push(*prev_tx_hash);
                            sandwich.possible_backrun = root.tx_hash;
                            sandwich.victims.push(frontrun_victims);
                        }
                    }
                }

                *prev_tx_hash = root.tx_hash;
                possible_victims.insert(root.tx_hash, vec![]);
            }
        }

        // Now, for each existing entry in possible_victims, we add the current
        // transaction hash as a potential victim, if it is not the same as
        // the key (which represents another transaction hash)
        for (k, v) in possible_victims.iter_mut() {
            if k != &root.tx_hash {
                v.push(root.tx_hash);
            }
        }
    }

    possible_sandwiches.values().cloned().collect()
}

//TODO: Add support for this type of flashloan sandwich

#[cfg(test)]
mod tests {

    use alloy_primitives::hex;
    use brontes_types::constants::WETH_ADDRESS;

    use super::*;
    use crate::{
        test_utils::{InspectorTestUtils, InspectorTxRunConfig, USDC_ADDRESS},
        Inspectors,
    };

    #[brontes_macros::test]
    async fn test_sandwich_different_eoa() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 1.0).await;

        let config = InspectorTxRunConfig::new(Inspectors::Sandwich)
            .with_mev_tx_hashes(vec![
                hex!("ff79c471b191c0021cfb62408cb1d7418d09334665a02106191f6ed16a47e36c").into(),
                hex!("19122ffe65a714f0551edbb16a24551031056df16ccaab39db87a73ac657b722").into(),
                hex!("67771f2e3b0ea51c11c5af156d679ccef6933db9a4d4d6cd7605b4eee27f9ac8").into(),
            ])
            .with_dex_prices()
            .needs_token(Address::new(hex!("28cf5263108c1c40cf30e0fe390bd9ccf929bf82")))
            .with_gas_paid_usd(16.64)
            .with_expected_profit_usd(15.648);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    // annoying
    async fn test_sandwich_part_of_jit_sandwich_simple() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 1.0).await;

        let config = InspectorTxRunConfig::new(Inspectors::Sandwich)
            .with_block(18500018)
            .with_dex_prices()
            .needs_token(hex!("8642a849d0dcb7a15a974794668adcfbe4794b56").into())
            .with_gas_paid_usd(40.26)
            .with_expected_profit_usd(1.18);

        inspector_util
            .run_inspector(
                config,
                Some(Box::new(|b| {
                    let BundleData::Sandwich(ref s) = b.data else { unreachable!() };
                    assert!(
                        s.frontrun_swaps[0].len() == 6,
                        "incorrect amount of frontrun swaps {:#?}",
                        s.frontrun_swaps[0]
                    );
                    assert!(
                        s.backrun_swaps.len() == 2,
                        "incorrect amount of backrun swaps {:#?}",
                        s.backrun_swaps
                    );

                    assert!(
                        s.victim_swaps_tx_hashes.iter().flatten().count() == 2,
                        "incorrect amount of victims"
                    );
                })),
            )
            .await
            .unwrap();
    }

    /// this is a jit sandwich
    #[brontes_macros::test]
    async fn test_sandwich_part_of_jit_sandwich() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 1.0).await;

        let config = InspectorTxRunConfig::new(Inspectors::Sandwich)
            .with_dex_prices()
            .with_mev_tx_hashes(vec![
                hex!("22ea36d516f59cc90ccc01042e20f8fba196f32b067a7e5f1510099140ae5e0a").into(),
                hex!("72eb3269ac013cf663dde9aa11cc3295e0dfb50c7edfcf074c5c57b43611439c").into(),
                hex!("3b4138bac9dc9fa4e39d8d14c6ecd7ec0144fe26b120ea799317aa15fa35ddcd").into(),
                hex!("99785f7b76a9347f13591db3574506e9f718060229db2826b4925929ebaea77e").into(),
                hex!("31dedbae6a8e44ec25f660b3cd0e04524c6476a0431ab610bb4096f82271831b").into(),
            ])
            .needs_tokens(vec![
                hex!("b17548c7b510427baac4e267bea62e800b247173").into(),
                hex!("ed4e879087ebd0e8a77d66870012b5e0dffd0fa4").into(),
                hex!("50D1c9771902476076eCFc8B2A83Ad6b9355a4c9").into(),
            ])
            .with_gas_paid_usd(90.875025)
            .with_expected_profit_usd(13.6);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_big_mac_sandwich() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 1.0).await;

        let config = InspectorTxRunConfig::new(Inspectors::Sandwich)
            .with_dex_prices()
            .with_mev_tx_hashes(vec![
                hex!("2a187ed5ba38cc3b857726df51ce99ee6e29c9bcaa02be1a328f99c3783b3303").into(),
                hex!("7325392f41338440f045cb1dba75b6099f01f8b00983e33cc926eb27aacd7e2d").into(),
                hex!("bcb8115fb54b7d6b0a0b0faf6e65fae02066705bd4afde70c780d4251a771428").into(),
                hex!("0b428553bc2ccc8047b0da46e6c1c1e8a338d9a461850fcd67ddb233f6984677").into(),
                hex!("fb2ef488bf7b6ad09accb126330837198b0857d2ea0052795af520d470eb5e1d").into(),
            ])
            .needs_tokens(vec![
                WETH_ADDRESS,
                hex!("dac17f958d2ee523a2206206994597c13d831ec7").into(),
            ])
            .with_gas_paid_usd(21.9)
            .with_expected_profit_usd(0.015);

        inspector_util
            .run_inspector(
                config,
                Some(Box::new(|bundle: &Bundle| {
                    let BundleData::Sandwich(ref sando) = bundle.data else {
                        panic!("given bundle wasn't a sandwich");
                    };
                    assert!(sando.frontrun_tx_hash.len() == 2, "didn't find the big mac");
                })),
            )
            .await
            .unwrap();
    }

    #[brontes_macros::test]
    async fn test_related_victim_tx_sandwich() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 1.0).await;

        let config = InspectorTxRunConfig::new(Inspectors::Sandwich)
            .with_dex_prices()
            .with_mev_tx_hashes(vec![
                hex!("561dc89f55be726eb4a6e42b811b514391d6f5619ac54a2b3546f4a3ce747e98").into(),
                hex!("efc9bcea246c70f4e915cb26a62019325d73871dbb31849cbf7541a5bc069f1c").into(),
                hex!("17a8ebe7b7d153d123b27714570bc5a7d1ead669cd90f9e13654a46542ed4367").into(),
                hex!("bf18530786a7ddf9da5316e57f0f041de09e149a42a121edd532f5ce3bb1cc4b").into(),
                hex!("a51d90663cbf127440972163d3943d18e3a79dae9a77e065b0980f8d192b65e7").into(),
                hex!("3b0a069a010d5ebb00be9d4cc86d4dce90687d41eacfd05f1916d12b061e24f2").into(),
            ])
            .needs_tokens(vec![
                WETH_ADDRESS,
                hex!("628a3b2e302c7e896acc432d2d0dd22b6cb9bc88").into(),
                hex!("d9016a907dc0ecfa3ca425ab20b6b785b42f2373").into(),
                hex!("8390a1da07e376ef7add4be859ba74fb83aa02d5").into(),
                hex!("51cb253744189f11241becb29bedd3f1b5384fdb").into(),
                USDC_ADDRESS,
            ])
            .with_gas_paid_usd(61.0)
            .with_expected_profit_usd(1.18);

        inspector_util
            .run_inspector(
                config,
                Some(Box::new(|bundle: &Bundle| {
                    let BundleData::Sandwich(ref sando) = bundle.data else {
                        panic!("expected a sandwich");
                    };
                    // assert that we didn't drop the non related sando
                    assert_eq!(sando.victim_swaps_tx_hashes.iter().flatten().count(), 4);
                })),
            )
            .await
            .unwrap();
    }

    #[brontes_macros::test]
    async fn test_low_profit_sandwich1() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 1.0).await;

        let config = InspectorTxRunConfig::new(Inspectors::Sandwich)
            .with_dex_prices()
            .with_mev_tx_hashes(vec![
                hex!("73003ef0efa2d7fea8b54418d58c529fe02dfa7f074c792f608c52028671c0ee").into(),
                hex!("9a52628d5f1b4129ee85768cf96477824c158ebce48b4331ab4f89de28a39ef1").into(),
                hex!("a46bfbd85fbcaf8450879d73f27436bf942078e5762af68bc10757745b5e1c9a").into(),
            ])
            .needs_tokens(vec![
                WETH_ADDRESS,
                hex!("8390a1da07e376ef7add4be859ba74fb83aa02d5").into(),
            ])
            .with_gas_paid_usd(16.57)
            .with_expected_profit_usd(0.001);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_low_profit_sandwich2() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 1.0).await;

        let config = InspectorTxRunConfig::new(Inspectors::Sandwich)
            .with_dex_prices()
            .with_mev_tx_hashes(vec![
                hex!("3c1592d19a18c7237d6e42ca1541bc82bce4789600f288d933c7476cdd20f375").into(),
                hex!("b53dfdce0e49609f58df3a229bd431ba8f9d2d201ba4a0ccd40ae11024b8c333").into(),
                hex!("9955b95cc97a07fab9b42fdb675560256a35feaa8ce98292b594c88d218ebb9d").into(),
                hex!("287d48d4841cb8cc34771d2df2f00e42ee31711910358d372b4b546cad44679c").into(),
            ])
            .needs_tokens(vec![
                WETH_ADDRESS,
                hex!("4309e88d1d511f3764ee0f154cee98d783b61f09").into(),
                hex!("6bc40d4099f9057b23af309c08d935b890d7adc0").into(),
            ])
            .with_gas_paid_usd(30.0)
            .with_expected_profit_usd(0.03);

        inspector_util.run_inspector(config, None).await.unwrap();
    }
}
