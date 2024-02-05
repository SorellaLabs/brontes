use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    hash::Hash,
    sync::Arc,
};

use brontes_database::libmdbx::LibmdbxReader;
use brontes_types::{
    db::dex::PriceAt,
    mev::{Bundle, BundleData, MevType, Sandwich},
    normalized_actions::{Actions, NormalizedSwap},
    tree::{BlockTree, GasDetails, Node, TxInfo},
    ToFloatNearest,
};
use itertools::Itertools;
use reth_primitives::{Address, B256};

use crate::{shared_utils::SharedInspectorUtils, Inspector, MetadataCombined};

pub struct SandwichInspector<'db, DB: LibmdbxReader> {
    inner: SharedInspectorUtils<'db, DB>,
}

impl<'db, DB: LibmdbxReader> SandwichInspector<'db, DB> {
    pub fn new(quote: Address, db: &'db DB) -> Self {
        Self { inner: SharedInspectorUtils::new(quote, db) }
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

#[async_trait::async_trait]
impl<DB: LibmdbxReader> Inspector for SandwichInspector<'_, DB> {
    async fn process_tree(
        &self,
        tree: Arc<BlockTree<Actions>>,
        metadata: Arc<MetadataCombined>,
    ) -> Vec<Bundle> {
        let search_fn = |node: &Node<Actions>| {
            (
                node.data.is_swap() || node.data.is_transfer(),
                node.subactions
                    .iter()
                    .any(|action| action.is_swap() || action.is_transfer()),
            )
        };

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
                    let vicitim_info = victims
                        .iter()
                        .map(|victims| {
                            victims
                                .into_iter()
                                .map(|v| tree.get_tx_info(*v).unwrap())
                                .collect::<Vec<_>>()
                        })
                        .collect_vec();

                    let victim_actions = victims
                        .iter()
                        .map(|victim| {
                            victim
                                .into_iter()
                                .map(|v| tree.collect(*v, search_fn.clone()))
                                .collect::<Vec<_>>()
                        })
                        .collect::<Vec<_>>();

                    if victim_actions
                        .iter()
                        .any(|inner| inner.iter().any(|s| s.is_empty()))
                    {
                        return None
                    }

                    if victims
                        .iter()
                        .flatten()
                        .map(|v| tree.get_root(*v).unwrap().head.data.clone())
                        .any(|d| d.is_revert() || mev_executor_contract == d.get_to_address())
                    {
                        return None
                    }

                    let frontrun_info = possible_frontruns
                        .iter()
                        .flat_map(|pf| tree.get_tx_info(*pf))
                        .collect::<Vec<_>>();

                    let back_run_info = tree.get_tx_info(possible_backrun)?;

                    let searcher_actions = possible_frontruns
                        .iter()
                        .chain(vec![&possible_backrun])
                        .map(|tx| tree.collect(*tx, search_fn.clone()))
                        .filter(|f| !f.is_empty())
                        .collect::<Vec<Vec<Actions>>>();

                    self.calculate_sandwich(
                        metadata.clone(),
                        frontrun_info,
                        back_run_info,
                        searcher_actions,
                        vicitim_info,
                        victim_actions,
                    )
                },
            )
            .collect::<Vec<_>>()
    }
}

impl<DB: LibmdbxReader> SandwichInspector<'_, DB> {
    fn calculate_sandwich(
        &self,
        metadata: Arc<MetadataCombined>,
        mut possible_front_runs_info: Vec<TxInfo>,
        backrun_info: TxInfo,
        mut searcher_actions: Vec<Vec<Actions>>,
        // victims
        mut victim_info: Vec<Vec<TxInfo>>,
        mut victim_actions: Vec<Vec<Vec<Actions>>>,
    ) -> Option<Bundle> {
        let all_actions = searcher_actions.clone();
        let back_run_swaps = searcher_actions
            .pop()?
            .iter()
            .filter(|s| s.is_swap())
            .map(|s| s.clone().force_swap())
            .collect_vec();

        let front_run_swaps = searcher_actions
            .iter()
            .map(|actions| {
                actions
                    .into_iter()
                    .filter(|s| s.is_swap())
                    .map(|s| s.clone().force_swap())
                    .collect_vec()
            })
            .collect_vec();

        //TODO: Check later if this method correctly identifies an incorrect middle
        // frontrun that is unrelated
        if !Self::has_pool_overlap(&front_run_swaps, &back_run_swaps, &victim_actions) {
            // if we don't satisfy a sandwich but we have more than 1 possible front run
            // tx remaining, lets remove the false positive backrun tx and try again
            if possible_front_runs_info.len() > 1 {
                // remove dropped sandwiches
                victim_info.pop()?;
                victim_actions.pop()?;
                let back_run_info = possible_front_runs_info.pop()?;

                return self.calculate_sandwich(
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
        tracing::info!(?front_run_swaps, ?back_run_swaps, "swaps");

        let victim_swaps = victim_actions
            .iter()
            .flatten()
            .map(|tx_actions| {
                tx_actions
                    .iter()
                    .filter(|action| action.is_swap())
                    .map(|f| f.clone().force_swap())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

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
            .chain(vec![backrun_info.gas_details].iter())
            .map(|g| g.gas_paid())
            .sum::<u128>();

        let gas_used = metadata.get_gas_price_usd(gas_used);
        let rev_usd = self.inner.get_dex_revenue_usd(
            backrun_info.tx_index,
            PriceAt::After,
            &all_actions,
            metadata.clone(),
        )?;

        let profit_usd = (rev_usd - &gas_used).to_float();

        let header = self.inner.build_bundle_header(
            &possible_front_runs_info[0],
            profit_usd,
            PriceAt::After,
            &all_actions,
            &possible_front_runs_info
                .iter()
                .chain(vec![backrun_info].iter())
                .map(|info| info.gas_details)
                .collect(),
            metadata,
            MevType::Sandwich,
        );

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
        println!("{:#?}, {:#?}", header, sandwich);

        Some(Bundle { header, data: BundleData::Sandwich(sandwich) })
    }

    fn has_pool_overlap(
        front_run_swaps: &Vec<Vec<NormalizedSwap>>,
        back_run_swaps: &Vec<NormalizedSwap>,
        victim_actions: &Vec<Vec<Vec<Actions>>>,
    ) -> bool {
        //  check for pool overlap
        let mut pools = HashSet::new();
        for swap in front_run_swaps.iter().flatten() {
            pools.insert(swap.pool);
        }

        let has_victim = victim_actions
            .iter()
            .flatten()
            .flatten()
            .filter(|action| action.is_swap())
            .map(|f| f.force_swap_ref().pool)
            .filter(|f| pools.contains(f))
            .collect::<HashSet<_>>();

        back_run_swaps
            .iter()
            .any(|inner| pools.contains(&inner.pool) && has_victim.contains(&inner.pool))
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
        let combined_results = result_senders
            .into_iter()
            .chain(result_contracts.into_iter());
        let unique_results: HashSet<_> = combined_results.collect();

        unique_results.into_iter().collect()
    }
}

fn get_possible_sandwich_duplicate_senders(tree: Arc<BlockTree<Actions>>) -> Vec<PossibleSandwich> {
    let mut duplicate_senders: HashMap<Address, B256> = HashMap::new();
    let mut possible_victims: HashMap<B256, Vec<B256>> = HashMap::new();
    let mut possible_sandwiches: HashMap<Address, PossibleSandwich> = HashMap::new();

    for root in tree.tx_roots.iter() {
        if root.head.data.is_revert() {
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
                    if !frontrun_victims.is_empty() {
                        match possible_sandwiches.entry(root.head.address) {
                            Entry::Vacant(e) => {
                                e.insert(PossibleSandwich {
                                    eoa:                   root.head.address,
                                    possible_frontruns:    vec![prev_tx_hash],
                                    possible_backrun:      root.tx_hash,
                                    mev_executor_contract: root.head.data.get_to_address(),
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
    let mut duplicate_mev_contracts: HashMap<Address, (B256, Address)> = HashMap::new();
    let mut possible_victims: HashMap<B256, Vec<B256>> = HashMap::new();
    let mut possible_sandwiches: HashMap<Address, PossibleSandwich> = HashMap::new();

    for root in tree.tx_roots.iter() {
        if root.head.data.is_revert() {
            continue
        }

        match duplicate_mev_contracts.entry(root.head.data.get_to_address()) {
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
                    if !frontrun_victims.is_empty() {
                        match possible_sandwiches.entry(root.head.data.get_to_address()) {
                            Entry::Vacant(e) => {
                                e.insert(PossibleSandwich {
                                    eoa:                   *frontrun_eoa,
                                    possible_frontruns:    vec![*prev_tx_hash],
                                    possible_backrun:      root.tx_hash,
                                    mev_executor_contract: root.head.data.get_to_address(),
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

#[cfg(test)]
mod tests {

    use alloy_primitives::hex;
    use serial_test::serial;

    use super::*;
    use crate::{
        test_utils::{InspectorTestUtils, InspectorTxRunConfig, USDC_ADDRESS},
        Inspectors,
    };

    #[tokio::test]
    #[serial]
    async fn test_sandwich_different_contract_address() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 1.0);

        let config = InspectorTxRunConfig::new(Inspectors::Sandwich)
            .with_mev_tx_hashes(vec![
                hex!("849c3cb1f299fa181e12b0506166e4aa221fce4384a710ac0d2e064c9b4e1c42").into(),
                hex!("055f8dd4eb02c15c1c1faa9b65da5521eaaff54f332e0fa311bc6ce6a4149d18").into(),
                hex!("ab765f128ae604fdf245c78c8d0539a85f0cf5dc7f83a2756890dea670138506").into(),
                hex!("06424e50ee53df1e06fa80a741d1549224e276aed08c3674b65eac9e97a39c45").into(),
                hex!("c0422b6abac94d29bc2a752aa26f406234d45e4f52256587be46255f7b861893").into(),
            ])
            .with_dex_prices()
            .with_gas_paid_usd(34.3368)
            .with_expected_profit_usd(24.0);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_sandwich_different_eoa() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 1.0);

        let config = InspectorTxRunConfig::new(Inspectors::Sandwich)
            .with_mev_tx_hashes(vec![
                hex!("ff79c471b191c0021cfb62408cb1d7418d09334665a02106191f6ed16a47e36c").into(),
                hex!("19122ffe65a714f0551edbb16a24551031056df16ccaab39db87a73ac657b722").into(),
                hex!("67771f2e3b0ea51c11c5af156d679ccef6933db9a4d4d6cd7605b4eee27f9ac8").into(),
            ])
            .with_dex_prices()
            .with_gas_paid_usd(16.64)
            .with_expected_profit_usd(15.648);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_sandwich_part_of_jit_sandwich_simple() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 1.0);

        let config = InspectorTxRunConfig::new(Inspectors::Sandwich)
            .with_mev_tx_hashes(vec![
                hex!("a203940b1d15c1c395b4b05cef9f0a05bf3c4a29fdb1bed47baddeac866e3729").into(),
                hex!("af2143d2448a2e639637f9184bc2539428230226c281a174ba4ef4ef00e00220").into(),
                hex!("3e9c6cbee7c8c85a3c1bbc0cc8b9e23674f86bc7aedc51f05eb9d0eda0f6247e").into(),
                hex!("9ee36a8a24c3eb5406e7a651525bcfbd0476445bd291622f89ebf8d13d54b7ee").into(),
            ])
            .with_dex_prices()
            .with_gas_paid_usd(40.26)
            .with_expected_profit_usd(-56.444);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_sandwich_part_of_jit_sandwich() {
        // this is a jit sandwich
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 1.0);

        let config = InspectorTxRunConfig::new(Inspectors::Sandwich)
            .with_dex_prices()
            .with_mev_tx_hashes(vec![
                hex!("22ea36d516f59cc90ccc01042e20f8fba196f32b067a7e5f1510099140ae5e0a").into(),
                hex!("72eb3269ac013cf663dde9aa11cc3295e0dfb50c7edfcf074c5c57b43611439c").into(),
                hex!("3b4138bac9dc9fa4e39d8d14c6ecd7ec0144fe26b120ea799317aa15fa35ddcd").into(),
                hex!("99785f7b76a9347f13591db3574506e9f718060229db2826b4925929ebaea77e").into(),
                hex!("31dedbae6a8e44ec25f660b3cd0e04524c6476a0431ab610bb4096f82271831b").into(),
            ])
            .with_gas_paid_usd(90.875025)
            .with_expected_profit_usd(-9.003);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_big_mac_sandwich() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 1.0);

        let config = InspectorTxRunConfig::new(Inspectors::Sandwich)
            .with_dex_prices()
            .with_mev_tx_hashes(vec![
                hex!("2a187ed5ba38cc3b857726df51ce99ee6e29c9bcaa02be1a328f99c3783b3303").into(),
                hex!("7325392f41338440f045cb1dba75b6099f01f8b00983e33cc926eb27aacd7e2d").into(),
                hex!("bcb8115fb54b7d6b0a0b0faf6e65fae02066705bd4afde70c780d4251a771428").into(),
                hex!("0b428553bc2ccc8047b0da46e6c1c1e8a338d9a461850fcd67ddb233f6984677").into(),
                hex!("fb2ef488bf7b6ad09accb126330837198b0857d2ea0052795af520d470eb5e1d").into(),
            ])
            .with_gas_paid_usd(21.9)
            .with_expected_profit_usd(0.015);

        inspector_util
            .run_inspector(
                config,
                Some(Box::new(|bundle: &Bundle| {
                    let BundleData::Sandwich(ref sando) = bundle.data else {
                        assert!(false, "given bundle wasn't a sandwich");
                        return
                    };
                    assert!(sando.frontrun_tx_hash.len() == 2, "didn't find the big mac");
                })),
            )
            .await
            .unwrap();
    }
}
