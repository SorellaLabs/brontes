use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    sync::Arc,
};

use brontes_database::Metadata;
use brontes_database_libmdbx::Libmdbx;
use brontes_types::{
    classified_mev::{MevType, Sandwich, SpecificMev},
    normalized_actions::Actions,
    tree::{BlockTree, GasDetails, Node},
    ToFloatNearest,
};
use itertools::Itertools;
use malachite::{num::basic::traits::Zero, Rational};
use reth_primitives::{Address, B256};

use crate::{shared_utils::SharedInspectorUtils, ClassifiedMev, Inspector};

pub struct SandwichInspector<'db> {
    inner: SharedInspectorUtils<'db>,
}

impl<'db> SandwichInspector<'db> {
    pub fn new(quote: Address, db: &'db Libmdbx) -> Self {
        Self { inner: SharedInspectorUtils::new(quote, db) }
    }
}

#[derive(Debug)]
pub struct PossibleSandwich {
    eoa:                   Address,
    tx0:                   B256,
    tx1:                   B256,
    mev_executor_contract: Address,
    victims:               Vec<B256>,
}

#[async_trait::async_trait]
impl Inspector for SandwichInspector<'_> {
    async fn process_tree(
        &self,
        tree: Arc<BlockTree<Actions>>,
        meta_data: Arc<Metadata>,
    ) -> Vec<(ClassifiedMev, Box<dyn SpecificMev>)> {
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

impl SandwichInspector<'_> {
    fn calculate_sandwich(
        &self,
        idx: usize,
        eoa: Address,
        mev_executor_contract: Address,
        metadata: Arc<Metadata>,
        txes: [B256; 2],
        searcher_gas_details: [GasDetails; 2],
        mut searcher_actions: Vec<Vec<Actions>>,
        // victim
        victim_txes: Vec<B256>,
        victim_actions: Vec<Vec<Actions>>,
        victim_gas: Vec<GasDetails>,
    ) -> Option<(ClassifiedMev, Box<dyn SpecificMev>)> {
        if searcher_actions.len() < 2 {
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

        let frontrun_swaps = searcher_actions
            .remove(0)
            .into_iter()
            .filter(|s| s.is_swap())
            .map(|s| s.force_swap())
            .collect_vec();

        let backrun_swaps = searcher_actions
            .remove(searcher_actions.len() - 1)
            .into_iter()
            .filter(|s| s.is_swap())
            .map(|s| s.force_swap())
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
            /*
            frontrun_swaps_index:      frontrun_swaps
                .iter()
                .map(|s| s.trace_index)
                .collect::<Vec<_>>(),
            frontrun_swaps_from:       frontrun_swaps.iter().map(|s| s.from).collect::<Vec<_>>(),
            frontrun_swaps_pool:       frontrun_swaps.iter().map(|s| s.pool).collect::<Vec<_>>(),
            frontrun_swaps_token_in:   frontrun_swaps
                .iter()
                .map(|s| s.token_in)
                .collect::<Vec<_>>(),
            frontrun_swaps_token_out:  frontrun_swaps
                .iter()
                .map(|s| s.token_out)
                .collect::<Vec<_>>(),
            frontrun_swaps_amount_in:  frontrun_swaps
                .iter()
                .map(|s| s.amount_in.to())
                .collect::<Vec<_>>(),
            frontrun_swaps_amount_out: frontrun_swaps
                .iter()
                .map(|s| s.amount_out.to())
                .collect::<Vec<_>>(),

            victim_tx_hashes:        victim_txes.clone(),
            victim_swaps_tx_hash:    victim_txes,
            victim_swaps_index:      victim_actions
                .iter()
                .flat_map(|swap| {
                    swap.into_iter()
                        .filter(|s| s.is_swap())
                        .map(|s| s.clone().force_swap().trace_index)
                        .collect_vec()
                })
                .collect(),
            victim_swaps_from:       victim_actions
                .iter()
                .flat_map(|swap| {
                    swap.into_iter()
                        .filter(|s| s.is_swap())
                        .map(|s| s.clone().force_swap().from)
                        .collect_vec()
                })
                .collect(),
            victim_swaps_pool:       victim_actions
                .iter()
                .flat_map(|swap| {
                    swap.into_iter()
                        .filter(|s| s.is_swap())
                        .map(|s| s.clone().force_swap().pool)
                        .collect_vec()
                })
                .collect(),
            victim_swaps_token_in:   victim_actions
                .iter()
                .flat_map(|swap| {
                    swap.into_iter()
                        .filter(|s| s.is_swap())
                        .map(|s| s.clone().force_swap().token_in)
                        .collect_vec()
                })
                .collect(),
            victim_swaps_token_out:  victim_actions
                .iter()
                .flat_map(|swap| {
                    swap.into_iter()
                        .filter(|s| s.is_swap())
                        .map(|s| s.clone().force_swap().token_out)
                        .collect_vec()
                })
                .collect(),
            victim_swaps_amount_in:  victim_actions
                .iter()
                .flat_map(|swap| {
                    swap.into_iter()
                        .filter(|s| s.is_swap())
                        .map(|s| s.clone().force_swap().amount_in.to())
                        .collect_vec()
                })
                .collect(),
            victim_swaps_amount_out: victim_actions
                .iter()
                .flat_map(|swap| {
                    swap.into_iter()
                        .filter(|s| s.is_swap())
                        .map(|s| s.clone().force_swap().amount_out.to())
                        .collect_vec()
                })
                .collect(),

            victim_gas_details_coinbase_transfer: victim_gas
                .iter()
                .map(|g| g.coinbase_transfer)
                .collect(),
            victim_gas_details_priority_fee: victim_gas.iter().map(|g| g.priority_fee).collect(),
            victim_gas_details_gas_used: victim_gas.iter().map(|g| g.gas_used).collect(),
            victim_gas_details_effective_gas_price: victim_gas
                .iter()
                .map(|g| g.effective_gas_price)
                .collect(),
            backrun_tx_hash: txes[1],
            backrun_gas_details: searcher_gas_details[1],
            backrun_swaps_index: backrun_swaps
                .iter()
                .map(|s| s.trace_index)
                .collect::<Vec<_>>(),
            backrun_swaps_from: backrun_swaps.iter().map(|s| s.from).collect::<Vec<_>>(),
            backrun_swaps_pool: backrun_swaps.iter().map(|s| s.pool).collect::<Vec<_>>(),
            backrun_swaps_token_in: backrun_swaps.iter().map(|s| s.token_in).collect::<Vec<_>>(),
            backrun_swaps_token_out: backrun_swaps
                .iter()
                .map(|s| s.token_out)
                .collect::<Vec<_>>(),
            backrun_swaps_amount_in: backrun_swaps
                .iter()
                .map(|s| s.amount_in.to())
                .collect::<Vec<_>>(),
            backrun_swaps_amount_out: backrun_swaps
                .iter()
                .map(|s| s.amount_out.to())
                .collect::<Vec<_>>(),
                */
        };

        let classified_mev = ClassifiedMev {
            eoa,
            mev_profit_collector,
            tx_hash: txes[0],
            mev_contract: mev_executor_contract,
            block_number: metadata.block_num,
            mev_type: MevType::Sandwich,
            finalized_profit_usd: (rev_usd - &gas_used).to_float(),
            finalized_bribe_usd: gas_used.to_float(),
        };

        Some((classified_mev, Box::new(sandwich)))
    }

    fn get_possible_sandwich(&self, tree: Arc<BlockTree<Actions>>) -> Vec<PossibleSandwich> {
        let iter = tree.tx_roots.iter();
        if iter.len() < 3 {
            return vec![]
        }

        let mut set: Vec<PossibleSandwich> = Vec::new();
        let mut duplicate_senders: HashMap<Address, Vec<B256>> = HashMap::new();
        let mut possible_victims: HashMap<B256, Vec<B256>> = HashMap::new();

        for root in iter {
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
                            if victims.len() >= 2 {
                                // Create
                                set.push(PossibleSandwich {
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

        set
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, str::FromStr, time::SystemTime};

    use alloy_primitives::hex;
    use brontes_classifier::Classifier;
    use reth_primitives::U256;
    use serial_test::serial;
    use tokio::sync::mpsc::unbounded_channel;

    use super::*;
    use crate::test_utils::{InspectorTestUtils, InspectorTxRunConfig, USDC_ADDRESS};

    #[tokio::test]
    async fn test_sandwich() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 1.0);

        let config = InspectorTxRunConfig::new(MevType::Sandwich)
            .with_mev_tx_hashes(vec![
                hex!("ff79c471b191c0021cfb62408cb1d7418d09334665a02106191f6ed16a47e36c").into(),
                hex!("19122ffe65a714f0551edbb16a24551031056df16ccaab39db87a73ac657b722").into(),
                hex!("67771f2e3b0ea51c11c5af156d679ccef6933db9a4d4d6cd7605b4eee27f9ac8").into(),
            ])
            .with_dex_prices()
            .with_expected_gas_used(16.0)
            .with_expected_profit_usd(23.0);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[tokio::test]
    async fn test_complex_sandwich() {
        // this is a jit sandwich
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.1);
        let block_num = 18539312;

        let config = InspectorTxRunConfig::new(MevType::Sandwich)
            .with_dex_prices()
            .with_block(18539312)
            .with_expected_gas_used(90.875025)
            .with_expected_profit_usd(-24.031640);

        inspector_util.run_inspector(config, None).await.unwrap();
    }
}
