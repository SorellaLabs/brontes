use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};

use async_trait::async_trait;
use poirot_types::tree::GasDetails;
use reth_primitives::{Address, H256};

use crate::{Actions, ClassifiedMev, Inspector, Metadata, SpecificMev, TimeTree};

pub struct JitInspector;

#[async_trait]
impl Inspector for JitInspector {
    async fn process_tree(
        &self,
        tree: Arc<TimeTree<Actions>>,
        metadata: Arc<Metadata>,
    ) -> Vec<(ClassifiedMev, Box<dyn SpecificMev>)> {
        let iter = tree.roots.iter();
        if iter.len() < 3 {
            return vec![]
        }

        // could tech be more than one victim but unlikely
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
                        tree.inspect(*victim, |node| {
                            node.subactions
                                .iter()
                                .any(|action| action.is_swap() || action.is_transfer())
                        })
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                    })
                    .collect::<Vec<Vec<Actions>>>();

                let searcher_actions = vec![tx0, tx1]
                    .into_iter()
                    .flat_map(|tx| {
                        tree.inspect(tx, |node| {
                            node.subactions.iter().any(|action| {
                                action.is_mint() || action.is_burn() || action.is_transfer()
                            })
                        })
                    })
                    .collect::<Vec<Vec<Actions>>>();

                self.calculate_jit(
                    eoa,
                    mev_addr,
                    metadata.clone(),
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

impl JitInspector {
    fn calculate_jit(
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
        None
    }
}
