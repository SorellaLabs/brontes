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
use itertools::Itertools;
use reth_primitives::{Address, H256};
use tracing::error;

use crate::{shared_utils::SharedInspectorUtils, ClassifiedMev, Inspector};

#[derive(Default)]
pub struct SandwichInspector {
    inner: SharedInspectorUtils,
}

pub struct PossibleSandwich {
    eoa:      Address,
    tx0:      H256,
    tx1:      H256,
    mev_addr: Address,
    victims:   Vec<H256>,
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
        println!("roots len: {:?}", iter.len());
        if iter.len() < 3 {
            return vec![]
        }

        let mut sets: Vec<PossibleSandwich> = Vec::new();

        let mut duplicate_senders = HashMap::new();
        let mut possible_victims: HashMap<H256, Vec<H256>> = HashMap::new();\


        // We loop through all transactions in the block
        for root in iter {
            match duplicate_senders.entry(root.head.address) {
                // If we have not seen this sender before, we add the tx hash to the map
                Entry::Vacant(v) => {
                    v.insert(root.tx_hash);
                
                    possible_victims.insert(root.tx_hash, vec![]);
                }
                Entry::Occupied(mut o) => {
                    // if the sender has already been seen, get the tx hash of the previous tx
                    let tx0: H256 = *o.get();
                    if let Some(mut victims) = possible_victims.remove(&entry) {
                        if victims.len() < 2 {
                            o.insert(root.tx_hash);
                        } else {
                            o.insert(root.tx_hash);
                            let _ = victims.remove(0);
                            set.push(
                                PossibleSandwich {
                                    eoa: root.head.address,
                                    tx0,
                                    tx1: root.tx_hash,
                                    mev_addr: root.head.data.get_too_address(),
                                    victims,
                                }
                            );
                        }
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
                println!("\n\nFOUND SET: {:?}\n", (eoa, tx0, tx1, mev_addr, &victim));

                let gas = [
                    tree.get_gas_details(tx0).cloned().unwrap(),
                    tree.get_gas_details(tx1).cloned().unwrap(),
                ];
                println!("GAS: {:?}\n", gas);

                let victim_gas = victim
                    .iter()
                    .map(|victim| tree.get_gas_details(*victim).cloned().unwrap())
                    .collect::<Vec<_>>();

                println!("VICTIM GAS: {:?}\n", gas);

                let victim_actions = victim
                    .iter()
                    .map(|victim| {
                        tree.inspect(*victim, search_fn.clone())
                            .into_iter()
                            .flatten()
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<Vec<Actions>>>();

                println!("VICTIM ACTIONS: {:?}\n", victim_actions);

                let searcher_actions = vec![tx0, tx1]
                    .into_iter()
                    .flat_map(|tx| tree.inspect(tx, search_fn.clone()))
                    .collect::<Vec<Vec<Actions>>>();

                println!("SEARCHER ACTIONS: {:?}\n", searcher_actions);

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

        println!("{:?}", appearance_usd_deltas);

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

        let frontrun_swaps = searcher_actions
            .remove(0)
            .into_iter()
            .map(|s| s.force_swap())
            .collect_vec();
        let backrun_swaps = searcher_actions
            .remove(searcher_actions.len() - 1)
            .into_iter()
            .map(|s| s.force_swap())
            .collect_vec();

        let sandwich = Sandwich {
            frontrun_tx_hash:          txes[0],
            frontrun_gas_details:      searcher_gas_details[0],
            frontrun_swaps_index:      frontrun_swaps.iter().map(|s| s.index).collect::<Vec<_>>(),
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
            victim_swaps_tx_hash:    victim_txes
                .iter()
                .enumerate()
                .flat_map(|(idx, tx)| vec![*tx].repeat(searcher_actions[idx].len()))
                .collect_vec(),
            victim_swaps_index:      searcher_actions
                .iter()
                .flat_map(|swap| {
                    swap.into_iter()
                        .map(|s| s.clone().force_swap().index)
                        .collect_vec()
                })
                .collect(),
            victim_swaps_from:       searcher_actions
                .iter()
                .flat_map(|swap| {
                    swap.into_iter()
                        .map(|s| s.clone().force_swap().from)
                        .collect_vec()
                })
                .collect(),
            victim_swaps_pool:       searcher_actions
                .iter()
                .flat_map(|swap| {
                    swap.into_iter()
                        .map(|s| s.clone().force_swap().pool)
                        .collect_vec()
                })
                .collect(),
            victim_swaps_token_in:   searcher_actions
                .iter()
                .flat_map(|swap| {
                    swap.into_iter()
                        .map(|s| s.clone().force_swap().token_in)
                        .collect_vec()
                })
                .collect(),
            victim_swaps_token_out:  searcher_actions
                .iter()
                .flat_map(|swap| {
                    swap.into_iter()
                        .map(|s| s.clone().force_swap().token_out)
                        .collect_vec()
                })
                .collect(),
            victim_swaps_amount_in:  searcher_actions
                .iter()
                .flat_map(|swap| {
                    swap.into_iter()
                        .map(|s| s.clone().force_swap().amount_in.to())
                        .collect_vec()
                })
                .collect(),
            victim_swaps_amount_out: searcher_actions
                .iter()
                .flat_map(|swap| {
                    swap.into_iter()
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
            backrun_swaps_index: backrun_swaps.iter().map(|s| s.index).collect::<Vec<_>>(),
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
