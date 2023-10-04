use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};

use brontes_database::Metadata;
use brontes_types::{
    classified_mev::{MevType, Sandwich, SpecificMev},
    normalized_actions::Actions,
    tree::{GasDetails, Node, TimeTree},
    ToFloatNearest,
};
use malachite::{num::conversion::traits::RoundingFrom, rounding_modes::RoundingMode, Rational};
use reth_primitives::{Address, H256};
use tracing::error;

use crate::{shared_utils::SharedInspectorUtils, ClassifiedMev, Inspector};

#[derive(Default)]
pub struct SandwichInspector {
    inner: SharedInspectorUtils,
}

#[async_trait::async_trait]
impl Inspector for SandwichInspector {
    async fn process_tree(
        &self,
        tree: Arc<TimeTree<Actions>>,
        meta_data: Arc<Metadata>,
    ) -> Vec<(ClassifiedMev, Box<dyn SpecificMev>)> {
        // lets grab the set of all possible sandwich txes
        let iter = tree.roots.iter();
        if iter.len() < 3 {
            return vec![]
        }

        let mut set = Vec::new();
        let mut pairs = HashMap::new();
        let mut possible_victims: HashMap<H256, Vec<H256>> = HashMap::new();

        for root in iter {
            match pairs.entry(root.head.address) {
                Entry::Vacant(v) => {
                    v.insert(root.tx_hash);
                    possible_victims.insert(root.tx_hash, vec![]);
                }
                Entry::Occupied(o) => {
                    let entry: H256 = o.remove();
                    if let Some(victims) = possible_victims.remove(&entry) {
                        set.push((
                            root.head.address,
                            entry,
                            root.tx_hash,
                            root.head.data.get_too_address(),
                            victims,
                        ));
                    }
                }
            }

            possible_victims.iter_mut().for_each(|(_, v)| {
                v.push(root.tx_hash);
            });
        }

        let search_fn = |node: &Node<Actions>| {
            node.subactions
                .iter()
                .any(|action| action.is_swap() || action.is_transfer())
        };

        set.into_iter()
            .filter_map(|(eoa, tx0, tx1, mev_addr, victim)| {
                let gas = [
                    tree.get_gas_details(tx0).cloned().unwrap(),
                    tree.get_gas_details(tx1).cloned().unwrap(),
                ];

                let victim_gas = victim
                    .iter()
                    .map(|victim| tree.get_gas_details(*victim).cloned().unwrap())
                    .collect::<Vec<_>>();

                let victim_actions = victim
                    .iter()
                    .map(|victim| {
                        tree.inspect(*victim, search_fn.clone())
                            .into_iter()
                            .flatten()
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<Vec<Actions>>>();

                let searcher_actions = vec![tx0, tx1]
                    .into_iter()
                    .flat_map(|tx| tree.inspect(tx, search_fn.clone()))
                    .collect::<Vec<Vec<Actions>>>();

                self.calculate_sandwich(
                    eoa,
                    mev_addr,
                    meta_data.clone(),
                    [tx0, tx1],
                    gas,
                    searcher_actions,
                    victim,
                    victim_actions,
                    victim_gas,
                )
            })
            .collect::<Vec<_>>()
    }
}

impl SandwichInspector {
    fn calculate_sandwich(
        &self,
        eoa: Address,
        mev_addr: Address,
        metadata: Arc<Metadata>,
        txes: [H256; 2],
        searcher_gas_details: [GasDetails; 2],
        mut searcher_actions: Vec<Vec<Actions>>,
        // victim
        victim_txes: Vec<H256>,
        victim_actions: Vec<Vec<Actions>>,
        victim_gas: Vec<GasDetails>,
    ) -> Option<(ClassifiedMev, Box<dyn SpecificMev>)> {
        let deltas = self.inner.calculate_swap_deltas(&searcher_actions);

        let appearance_usd_deltas = self.inner.get_best_usd_delta(
            deltas.clone(),
            metadata.clone(),
            Box::new(|(appearance, _)| appearance),
        );

        let finalized_usd_deltas = self.inner.get_best_usd_delta(
            deltas,
            metadata.clone(),
            Box::new(|(_, finalized)| finalized),
        );

        let (finalized, appearance) = (finalized_usd_deltas?, appearance_usd_deltas?);

        if finalized.0 != appearance.0 {
            error!("finalized addr != appearance addr");
            return None
        }

        let gas_used = searcher_gas_details
            .iter()
            .map(|g| g.gas_paid())
            .sum::<u64>();

        let (gas_used_usd_appearance, gas_used_usd_finalized) =
            metadata.get_gas_price_usd(gas_used);

        let sandwich = Sandwich {
            front_run:             txes[0],
            front_run_gas_details: searcher_gas_details[0],
            front_run_swaps:       searcher_actions.remove(0),
            victim:                victim_txes,
            victim_gas_details:    victim_gas,
            victim_swaps:          victim_actions,
            back_run:              txes[1],
            back_run_gas_details:  searcher_gas_details[1],
            back_run_swaps:        searcher_actions.remove(0),
        };

        let classified_mev = ClassifiedMev {
            eoa,
            mev_profit_collector: finalized.0,
            tx_hash: txes[0],
            mev_contract: mev_addr,
            block_number: metadata.block_num,
            mev_type: MevType::Sandwich,
            submission_profit_usd: (appearance.1 - &gas_used_usd_appearance).to_float(),
            submission_bribe_usd: gas_used_usd_appearance.to_float(),
            finalized_profit_usd: (finalized.1 - &gas_used_usd_finalized).to_float(),
            finalized_bribe_usd: gas_used_usd_finalized.to_float(),
        };

        Some((classified_mev, Box::new(sandwich)))
    }
}
