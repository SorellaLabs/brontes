use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    sync::Arc,
};

use brontes_database::libmdbx::LibmdbxReader;
use brontes_types::{
    classified_mev::{MevType, Sandwich, SpecificMev},
    normalized_actions::Actions,
    tree::{BlockTree, GasDetails, Node},
    ToFloatNearest,
};
use itertools::Itertools;
use malachite::{num::basic::traits::Zero, Rational};
use reth_primitives::{Address, B256};

use crate::{shared_utils::SharedInspectorUtils, ClassifiedMev, Inspector, MetadataCombined};

pub struct SandwichInspector<'db, DB: LibmdbxReader> {
    inner: SharedInspectorUtils<'db, DB>,
}

impl<'db, DB: LibmdbxReader> SandwichInspector<'db, DB> {
    pub fn new(quote: Address, db: &'db DB) -> Self {
        Self { inner: SharedInspectorUtils::new(quote, db) }
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct PossibleSandwich {
    eoa:                   Address,
    tx0:                   B256,
    tx1:                   B256,
    mev_executor_contract: Address,
    victims:               Vec<B256>,
}

#[async_trait::async_trait]
impl<DB: LibmdbxReader> Inspector for SandwichInspector<'_, DB> {
    async fn process_tree(
        &self,
        tree: Arc<BlockTree<Actions>>,
        meta_data: Arc<MetadataCombined>,
    ) -> Vec<(ClassifiedMev, SpecificMev)> {
        // grab the set of all possible sandwich txes

        let search_fn = |node: &Node<Actions>| {
            (
                node.data.is_swap() || node.data.is_transfer(),
                node.subactions
                    .iter()
                    .any(|action| action.is_swap() || action.is_transfer()),
            )
        };

        self.get_possible_sandwich(tree.clone())
            .into_iter()
            .filter_map(|ps| {
                let gas = [
                    tree.get_gas_details(ps.tx0).cloned().unwrap(),
                    tree.get_gas_details(ps.tx1).cloned().unwrap(),
                ];

                let victim_gas = ps
                    .victims
                    .iter()
                    .map(|victim| tree.get_gas_details(*victim).cloned().unwrap())
                    .collect::<Vec<_>>();

                let victim_actions = ps
                    .victims
                    .iter()
                    .map(|victim| tree.collect(*victim, search_fn.clone()))
                    .collect::<Vec<Vec<Actions>>>();

                if victim_actions.iter().any(|inner| inner.is_empty()) {
                    return None
                }

                if ps
                    .victims
                    .iter()
                    .map(|v| tree.get_root(*v).unwrap().head.data.clone())
                    .filter(|d| !d.is_revert())
                    .any(|d| ps.mev_executor_contract == d.get_to_address())
                {
                    return None
                }

                let tx_idx = tree.get_root(ps.tx1).unwrap().position;

                let searcher_actions = vec![ps.tx0, ps.tx1]
                    .into_iter()
                    .map(|tx| tree.collect(tx, search_fn.clone()))
                    .filter(|f| !f.is_empty())
                    .collect::<Vec<Vec<Actions>>>();

                if searcher_actions.len() != 2 {
                    return None
                }

                self.calculate_sandwich(
                    tx_idx,
                    ps.eoa,
                    ps.mev_executor_contract,
                    meta_data.clone(),
                    [ps.tx0, ps.tx1],
                    gas,
                    searcher_actions,
                    ps.victims,
                    victim_actions,
                    victim_gas,
                )
            })
            .collect::<Vec<_>>()
    }
}

impl<DB: LibmdbxReader> SandwichInspector<'_, DB> {
    fn calculate_sandwich(
        &self,
        idx: usize,
        eoa: Address,
        mev_executor_contract: Address,
        metadata: Arc<MetadataCombined>,
        txes: [B256; 2],
        searcher_gas_details: [GasDetails; 2],
        searcher_actions: Vec<Vec<Actions>>,
        // victim
        victim_txes: Vec<B256>,
        victim_actions: Vec<Vec<Actions>>,
        victim_gas: Vec<GasDetails>,
    ) -> Option<(ClassifiedMev, SpecificMev)> {
        let frontrun_swaps = searcher_actions
            .get(0)?
            .into_iter()
            .filter(|s| s.is_swap())
            .map(|s| s.clone().force_swap())
            .collect_vec();

        let backrun_swaps = searcher_actions
            .get(searcher_actions.len() - 1)?
            .into_iter()
            .filter(|s| s.is_swap())
            .map(|s| s.clone().force_swap())
            .collect_vec();

        let mut pools = HashSet::new();

        for swap in &frontrun_swaps {
            pools.insert(swap.pool);
        }

        let has_victim = victim_actions
            .iter()
            .flatten()
            .filter(|action| action.is_swap())
            .map(|f| f.force_swap_ref().pool)
            .filter(|f| pools.contains(f))
            .collect::<HashSet<_>>();

        let victim_swaps = victim_actions
            .iter()
            .map(|tx_actions| {
                tx_actions
                    .iter()
                    .filter(|action| action.is_swap())
                    .map(|f| f.clone().force_swap())
                    .collect::<Vec<_>>()
            })
            .collect();

        if !backrun_swaps
            .iter()
            .any(|inner| pools.contains(&inner.pool) && has_victim.contains(&inner.pool))
        {
            return None
        }

        let deltas = self.inner.calculate_token_deltas(&searcher_actions);

        let addr_usd_deltas =
            self.inner
                .usd_delta_by_address(idx, deltas, metadata.clone(), false)?;

        let mev_profit_collector = self.inner.profit_collectors(&addr_usd_deltas);

        let rev_usd = addr_usd_deltas
            .values()
            .fold(Rational::ZERO, |acc, delta| acc + delta);

        let gas_used = searcher_gas_details
            .iter()
            .map(|g| g.gas_paid())
            .sum::<u128>();

        let gas_used = metadata.get_gas_price_usd(gas_used);

        let sandwich = Sandwich {
            frontrun_tx_hash: txes[0],
            frontrun_gas_details: searcher_gas_details[0],
            frontrun_swaps,
            victim_swaps_tx_hashes: victim_txes,
            victim_swaps,
            victim_swaps_gas_details: victim_gas,
            backrun_tx_hash: txes[1],
            backrun_swaps,
            backrun_gas_details: searcher_gas_details[1],
        };

        let classified_mev = ClassifiedMev {
            mev_tx_index: idx as u64,
            eoa,
            mev_profit_collector,
            tx_hash: txes[0],
            mev_contract: mev_executor_contract,
            block_number: metadata.block_num,
            mev_type: MevType::Sandwich,
            finalized_profit_usd: (rev_usd - &gas_used).to_float(),
            finalized_bribe_usd: gas_used.to_float(),
        };

        Some((classified_mev, SpecificMev::Sandwich(sandwich)))
    }

    fn get_possible_sandwich(&self, tree: Arc<BlockTree<Actions>>) -> Vec<PossibleSandwich> {
        let iter = tree.tx_roots.iter();
        if iter.len() < 3 {
            return vec![]
        }

        let mut set: HashSet<PossibleSandwich> = HashSet::new();
        let mut duplicate_mev_contracts: HashMap<Address, Vec<B256>> = HashMap::new();
        let mut duplicate_senders: HashMap<Address, Vec<B256>> = HashMap::new();
        let mut possible_victims: HashMap<B256, Vec<B256>> = HashMap::new();

        for root in iter {
            if root.head.data.is_revert() {
                continue
            }

            match duplicate_mev_contracts.entry(root.head.data.get_to_address()) {
                // If we have not seen this sender before, we insert the tx hash into the map
                Entry::Vacant(v) => {
                    v.insert(vec![root.tx_hash]);
                    possible_victims.insert(root.tx_hash, vec![]);
                }
                Entry::Occupied(mut o) => {
                    let prev_tx_hashes = o.get();

                    for prev_tx_hash in prev_tx_hashes {
                        // Find the victims between the previous and the current transaction
                        if let Some(victims) = possible_victims.get(prev_tx_hash) {
                            if victims.len() >= 1 {
                                // Create
                                set.insert(PossibleSandwich {
                                    eoa:                   root.head.address,
                                    tx0:                   *prev_tx_hash,
                                    tx1:                   root.tx_hash,
                                    mev_executor_contract: root.head.data.get_to_address(),
                                    victims:               victims.clone(),
                                });
                            }
                        }
                    }
                    // Add current transaction hash to the list of transactions for this sender
                    o.get_mut().push(root.tx_hash);
                    possible_victims.insert(root.tx_hash, vec![]);
                }
            }

            match duplicate_senders.entry(root.head.address) {
                // If we have not seen this sender before, we insert the tx hash into the map
                Entry::Vacant(v) => {
                    v.insert(vec![root.tx_hash]);
                    possible_victims.insert(root.tx_hash, vec![]);
                }
                Entry::Occupied(mut o) => {
                    let prev_tx_hashes = o.get();

                    for prev_tx_hash in prev_tx_hashes {
                        // Find the victims between the previous and the current transaction
                        if let Some(victims) = possible_victims.get(prev_tx_hash) {
                            if victims.len() >= 1 {
                                // Create
                                set.insert(PossibleSandwich {
                                    eoa:                   root.head.address,
                                    tx0:                   *prev_tx_hash,
                                    tx1:                   root.tx_hash,
                                    mev_executor_contract: root.head.data.get_to_address(),
                                    victims:               victims.clone(),
                                });
                            }
                        }
                    }
                    // Add current transaction hash to the list of transactions for this sender
                    o.get_mut().push(root.tx_hash);
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

        set.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, str::FromStr, time::SystemTime};

    use alloy_primitives::hex;
    use brontes_classifier::Classifier;
    use reth_primitives::U256;
    use serial_test::serial;

    use super::*;
    use crate::test_utils::{InspectorTestUtils, InspectorTxRunConfig, USDC_ADDRESS};

    #[tokio::test]
    #[serial]
    async fn test_sandwich_different_contract_address() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 1.0);

        let config = InspectorTxRunConfig::new(MevType::Sandwich)
            .with_mev_tx_hashes(vec![
                hex!("849c3cb1f299fa181e12b0506166e4aa221fce4384a710ac0d2e064c9b4e1c42").into(),
                hex!("055f8dd4eb02c15c1c1faa9b65da5521eaaff54f332e0fa311bc6ce6a4149d18").into(),
                hex!("ab765f128ae604fdf245c78c8d0539a85f0cf5dc7f83a2756890dea670138506").into(),
                hex!("06424e50ee53df1e06fa80a741d1549224e276aed08c3674b65eac9e97a39c45").into(),
                hex!("c0422b6abac94d29bc2a752aa26f406234d45e4f52256587be46255f7b861893").into(),
            ])
            .with_dex_prices()
            .with_expected_gas_used(34.3368)
            .with_expected_profit_usd(24.0);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_sandwich_different_eoa() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 1.0);

        let config = InspectorTxRunConfig::new(MevType::Sandwich)
            .with_mev_tx_hashes(vec![
                hex!("ff79c471b191c0021cfb62408cb1d7418d09334665a02106191f6ed16a47e36c").into(),
                hex!("19122ffe65a714f0551edbb16a24551031056df16ccaab39db87a73ac657b722").into(),
                hex!("67771f2e3b0ea51c11c5af156d679ccef6933db9a4d4d6cd7605b4eee27f9ac8").into(),
            ])
            .with_dex_prices()
            .with_expected_gas_used(16.64)
            .with_expected_profit_usd(15.648);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_sandwich_part_of_jit_sandwich_simple() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 1.0);

        let config = InspectorTxRunConfig::new(MevType::Sandwich)
            .with_mev_tx_hashes(vec![
                hex!("a203940b1d15c1c395b4b05cef9f0a05bf3c4a29fdb1bed47baddeac866e3729").into(),
                hex!("af2143d2448a2e639637f9184bc2539428230226c281a174ba4ef4ef00e00220").into(),
                hex!("3e9c6cbee7c8c85a3c1bbc0cc8b9e23674f86bc7aedc51f05eb9d0eda0f6247e").into(),
                hex!("9ee36a8a24c3eb5406e7a651525bcfbd0476445bd291622f89ebf8d13d54b7ee").into(),
            ])
            .with_dex_prices()
            .with_expected_gas_used(40.26)
            .with_expected_profit_usd(-56.444);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_sandwich_part_of_jit_sandwich() {
        // this is a jit sandwich
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 1.0);

        let config = InspectorTxRunConfig::new(MevType::Sandwich)
            .with_dex_prices()
            .with_mev_tx_hashes(vec![
                hex!("22ea36d516f59cc90ccc01042e20f8fba196f32b067a7e5f1510099140ae5e0a").into(),
                hex!("72eb3269ac013cf663dde9aa11cc3295e0dfb50c7edfcf074c5c57b43611439c").into(),
                hex!("3b4138bac9dc9fa4e39d8d14c6ecd7ec0144fe26b120ea799317aa15fa35ddcd").into(),
                hex!("99785f7b76a9347f13591db3574506e9f718060229db2826b4925929ebaea77e").into(),
                hex!("31dedbae6a8e44ec25f660b3cd0e04524c6476a0431ab610bb4096f82271831b").into(),
            ])
            .with_expected_gas_used(90.875025)
            .with_expected_profit_usd(-9.003);

        inspector_util.run_inspector(config, None).await.unwrap();
    }
    // TODO: write test for
    // 0x0ddca9d4baf7bff16b59a564e86c0a6d7e648771d2cfd43a022494bb9c9a8624 https://etherscan.io/tx/0x56fa3506eea903de8d548225708385b48407d3fccdbcd40e1554795b6157dcf0
}
