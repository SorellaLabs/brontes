use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    sync::Arc
};

use malachite::{num::conversion::traits::RoundingFrom, rounding_modes::RoundingMode, Rational};
use poirot_labeller::Metadata;
use poirot_types::{
    classified_mev::Sandwich,
    normalized_actions::{Actions, NormalizedSwap},
    tree::{GasDetails, Node, TimeTree}
};
use reth_primitives::{Address, H256};
use tracing::error;

use crate::{ClassifiedMev, Inspector};

pub struct SandwichInspector;

#[async_trait::async_trait]
impl Inspector for SandwichInspector {
    type Mev = Sandwich;

    async fn process_tree(
        &self,
        tree: Arc<TimeTree<Actions>>,
        meta_data: Arc<Metadata>
    ) -> (ClassifiedMev, Vec<Self::Mev>) {
        // lets grab the set of all possible sandwich txes
        let mut iter = tree.roots.iter();
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
                Entry::Occupied(mut o) => {
                    let entry: H256 = o.remove();
                    if let Some(victims) = possible_victims.remove(o) {
                        set.push((o, root.tx_hash, root.head.address, victims));
                    }
                }
            }

            possible_victims.iter_mut().for_each(|_, v| {
                v.push(root.tx_hash);
            });
        }

        let search_fn =
            |node: &Node<Actions>| node.subactions.iter().any(|action| action.is_swap());

        set.into_iter()
            .filter_map(|(tx0, tx1, mev_addr, victim)| {
                let gas = [
                    tree.get_gas_details(tx0).cloned().unwrap(),
                    tree.get_gas_details(tx1).cloned().unwrap()
                ];

                let victim_gas = victim
                    .iter()
                    .map(|victim| tree.get_gas_details(victim).cloned().unwrap())
                    .collect::<Vec<_>>();

                let victim_actions = victim
                    .iter()
                    .map(|victim| {
                        tree.inspect(victim, search_fn.clone())
                            .into_iter()
                            .flatten()
                            .map(|v| {
                                let Actions::Swap(s) = v else { panic!() };
                                s
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<Vec<NormalizedSwap>>>();

                let searcher_actions = vec![tx0, tx1]
                    .into_iter()
                    .map(|tx| {
                        tree.inspect(tx, search_fn.clone())
                            .into_iter()
                            .flatten()
                            .map(|v| {
                                let Actions::Swap(s) = v else { panic!() };
                                s
                            })
                            .collect()
                    })
                    .collect::<Vec<Vec<NormalizedSwap>>>();

                self.calculate_sandwich(
                    mev_addr,
                    meta_data.clone(),
                    [tx0, tx1],
                    gas,
                    searcher_actions,
                    victim,
                    victim_actions,
                    victim_gas
                )
            })
            .collect::<Vec<_>>()
    }
}

impl SandwichInspector {
    fn calculate_sandwich(
        &self,
        mev_addr: Address,
        metadata: Arc<Metadata>,
        txes: [H256; 2],
        searcher_gas_details: [GasDetails; 2],
        searcher_actions: Vec<Vec<NormalizedSwap>>,
        // victim
        victim_txes: Vec<H256>,
        victim_actions: Vec<Vec<NormalizedSwap>>,
        victim_gas: Vec<GasDetails>
    ) -> Option<(ClassifiedMev, Sandwich)> {
        let deltas = self.calculate_swap_deltas(&actions);

        let appearance_usd_deltas = self.get_best_usd_delta(
            deltas.clone(),
            metadata.clone(),
            Box::new(|(appearance, _)| appearance)
        );

        let finalized_usd_deltas =
            self.get_best_usd_delta(deltas, metadata.clone(), Box::new(|(_, finalized)| finalized));

        let (finalized, appearance) = (finalized_usd_deltas?, appearance_usd_deltas?);

        if finalized.0 != appearance.0 {
            error!("finalized addr != appearance addr");
            return None
        }

        let gas_used = gas_details.iter().map(|g| g.gas_paid()).sum::<u64>();

        let (gas_used_usd_appearance, gas_used_usd_finalized) = (
            Rational::from(gas_used) * &metadata.eth_prices.0,
            Rational::from(gas_used) * &metadata.eth_prices.1
        );

        let sandwich = Sandwich {
            front_run:             txes.0,
            front_run_gas_details: gas_details.0,
            front_run_swaps:       searcher_actions.remove(0)?,
            victim:                victim_txes,
            victim_gas_details:    victim_gas,
            victim_swaps:          victim_actions,
            back_run:              txes.1,
            back_run_gas_details:  gas_details.1,
            back_run_swaps:        searcher_actions.remove(0)?,
            mev_bot:               mev_addr
        };

        let classified_mev = ClassifiedMev {
            tx_hash: txes.0,
            mev_bot: mev_addr,
            block_number,
            mev_type: MevType::Sandwich,
            submission_profit_usd: f64::rounding_from(
                appearance.1 * &gas_used_usd_appearance,
                RoundingMode::Nearest
            ),
            submission_bribe_usd: f64::rounding_from(
                gas_used_usd_appearance,
                RoundingMode::Nearest
            ),
            finalized_profit_usd: f64::rounding_from(
                finalized.1 * &gas_used_usd_finalized,
                RoundingMode::Nearest
            ),
            finalized_bribe_usd: f64::rounding_from(gas_used_usd_finalized, RoundingMode::Nearest)
        };

        Some((classified_mev, sandwich))
    }
}
