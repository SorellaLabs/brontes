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

#[derive(Debug)]
pub struct PossibleSandwich {
    eoa:                   Address,
    tx0:                   H256,
    tx1:                   H256,
    mev_executor_contract: Address,
    victims:               Vec<H256>,
}

#[async_trait::async_trait]
impl Inspector for SandwichInspector {
    async fn process_tree(
        &self,
        tree: Arc<TimeTree<Actions>>,
        meta_data: Arc<Metadata>,
    ) -> Vec<(ClassifiedMev, Box<dyn SpecificMev>)> {
        // grab the set of all possible sandwich txes
        let iter = tree.roots.iter();
        println!("roots len: {:?}", iter.len());
        if iter.len() < 3 {
            return vec![];
        }

        let mut set: Vec<PossibleSandwich> = Vec::new();
        let mut duplicate_senders: HashMap<Address, Vec<H256>> = HashMap::new();
        let mut possible_victims: HashMap<H256, Vec<H256>> = HashMap::new();

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
                        if let Some(victims) = possible_victims.get(&prev_tx_hash) {
                            if victims.len() >= 2 {
                                // Create
                                set.push(PossibleSandwich {
                                    eoa:                   root.head.address,
                                    tx0:                   *prev_tx_hash,
                                    tx1:                   root.tx_hash,
                                    mev_executor_contract: root.head.data.get_too_address(),
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
        println!("{:#?}", set);

        let search_fn = |node: &Node<Actions>| {
            node.subactions
                .iter()
                .any(|action| action.is_swap() || action.is_transfer())
        };

        set.into_iter()
            .filter_map(|ps| {
                println!(
                    "\n\nFOUND SET: {:?}\n",
                    (ps.eoa, ps.tx0, ps.tx1, ps.mev_executor_contract, &ps.victims)
                );

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
                    .map(|victim| {
                        tree.inspect(*victim, search_fn.clone())
                            .into_iter()
                            .flatten()
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<Vec<Actions>>>();
                println!("Victim actions - {:#?}", victim_actions);

                let searcher_actions = vec![ps.tx0, ps.tx1]
                    .into_iter()
                    .flat_map(|tx| tree.inspect(tx, search_fn.clone()))
                    .collect::<Vec<Vec<Actions>>>();
                println!("Searcher actions - {:#?}", searcher_actions);

                self.calculate_sandwich(
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

impl SandwichInspector {
    fn calculate_sandwich(
        &self,
        eoa: Address,
        mev_executor_contract: Address,
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
        println!("deltas {:#?}", deltas);

        let appearance_usd_deltas = self.inner.get_best_usd_delta(
            deltas.clone(),
            metadata.clone(),
            Box::new(|(appearance, _)| appearance),
        );

        println!("appearance_usd_deltas {:#?}", appearance_usd_deltas);

        let finalized_usd_deltas = self.inner.get_best_usd_delta(
            deltas,
            metadata.clone(),
            Box::new(|(_, finalized)| finalized),
        );

        let (finalized, appearance) = (finalized_usd_deltas?, appearance_usd_deltas?);

        if finalized.0 != appearance.0 {
            println!("finalized addr != appearance addr");
            error!("finalized addr != appearance addr");
            return None;
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
            victim_swaps_tx_hash:    victim_txes,
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
            mev_contract: mev_executor_contract,
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

#[cfg(test)]
mod tests {
    use std::{str::FromStr, time::SystemTime};

    use brontes_classifier::Classifier;
    use brontes_core::test_utils::init_trace_parser;
    use brontes_database::database::Database;
    use brontes_types::test_utils::write_tree_as_json;
    use serial_test::serial;
    use tokio::sync::mpsc::unbounded_channel;

    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_sandwich() {
        dotenv::dotenv().ok();
        let block_num = 18522330;

        let (tx, _rx) = unbounded_channel();

        let tracer = init_trace_parser(tokio::runtime::Handle::current().clone(), tx);
        let db = Database::default();
        let classifier = Classifier::new();

        let block = tracer.execute_block(block_num).await.unwrap();
        let metadata = db.get_metadata(block_num).await;

        let tx = block.0.clone().into_iter().take(10).collect::<Vec<_>>();
        let tree = Arc::new(classifier.build_tree(tx, block.1, &metadata));

        write_tree_as_json(&tree, "./tree.json").await;

        let inspector = SandwichInspector::default();

        let t0 = SystemTime::now();
        let mev = inspector.process_tree(tree.clone(), metadata.into()).await;
        let t1 = SystemTime::now();
        let delta = t1.duration_since(t0).unwrap().as_micros();
        println!("sandwich inspector took: {} us", delta);

        // assert!(
        //     mev[0].0.tx_hash
        //         == H256::from_str(
        //
        // "0x80b53e5e9daa6030d024d70a5be237b4b3d5e05d30fdc7330b62c53a5d3537de"
        //         )
        //         .unwrap()
        // );

        println!("{:#?}", mev);
    }

    fn test_process_sandwich() {
        // let expected_sandwich = Sandwich {
        //     frontrun_tx_hash: H256::from_str(
        //         "0xd8d45bdcb25ba4cb2ecb357a5505d03fa2e67fe6e6cc032ca6c05de75d14f5b5",
        //     )
        //     .unwrap(),
        //     frontrun_gas_details: GasDetails {
        //         coinbase_transfer: 0, //todo
        //         priority_fee: 0,
        //         gas_used: 87336,
        //         effective_gas_price: 18.990569622,
        //     },
        //     frontrun_swaps_index: 0,
        //     frontrun_swaps_from: vec![
        //         H160::from_str("0xcc2687c14915fd68226ccf388842515739a739bd").
        // unwrap()     ],
        //     frontrun_swaps_pool: vec![
        //         H160::from_str("0xde55ec8002d6a3480be27e0b9755ef987ad6e151").
        // unwrap()     ],
        //     frontrun_swaps_token_in: vec![H160::from_str(
        //         "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
        //     )
        //     .unwrap()],
        //     frontrun_swaps_token_out: vec![H160::from_str(
        //         "0xdE55ec8002d6a3480bE27e0B9755EF987Ad6E151",
        //     )
        //     .unwrap()],
        //     frontrun_swaps_amount_in: vec![454788265862552718],
        //     frontrun_swaps_amount_out: vec![111888798809177],
        //     victim_tx_hashes: vec![H256::from_str(
        //         "0xfce96902655ca75f2da557c40e005ec74382fdaf9160c5492c48c49c283250ab",
        //     )
        //     .unwrap()],
        //     victim_swaps_tx_hash: vec![H256::from_str(
        //         "0xfce96902655ca75f2da557c40e005ec74382fdaf9160c5492c48c49c283250ab",
        //     )
        //     .unwrap()],
        //     victim_swaps_index: vec![1],
        //     victim_swaps_from: vec![
        //         H160::from_str("0x3fc91a3afd70395cd496c647d5a6cc9d4b2b7fad").
        // unwrap()     ],
        //     victim_swaps_pool: vec![
        //         H160::from_str("0xde55ec8002d6a3480be27e0b9755ef987ad6e151").
        // unwrap()     ],
        //     victim_swaps_token_in: vec![H160::from_str(
        //         "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
        //     )
        //     .unwrap()],
        //     victim_swaps_token_out: vec![H160::from_str(
        //         "0xdE55ec8002d6a3480bE27e0B9755EF987Ad6E151",
        //     )
        //     .unwrap()],
        //     victim_swaps_amount_in: vec![1000000000000000000],
        //     victim_swaps_amount_out: vec![206486606721996],
        //     victim_gas_details_coinbase_transfer: vec![0], //todo
        //     victim_gas_details_priority_fee: vec![100000000],
        //     victim_gas_details_gas_used: vec![100073],
        //     victim_gas_details_effective_gas_price: vec![18990569622],
        //     backrun_tx_hash: H256::from_str(
        //         "0x4479723b447600b2d577bf02bd409efab249985840463c8f7088e6b5a724c667",
        //     )
        //     .unwrap(),
        //     backrun_gas_details: GasDetails {
        //         coinbase_transfer: 0, //todo
        //         priority_fee: 0,
        //         gas_used: 84461,
        //         effective_gas_price: 18990569622,
        //     },
        //     backrun_swaps_index: 2,
        //     backrun_swaps_from: vec![
        //         H160::from_str("0xcc2687c14915fd68226ccf388842515739a739bd").
        // unwrap()     ],
        //     backrun_swaps_pool: vec![
        //         H160::from_str("0xde55ec8002d6a3480be27e0b9755ef987ad6e151").
        // unwrap()     ],
        //     backrun_swaps_token_in: vec![H160::from_str(
        //         "0xdE55ec8002d6a3480bE27e0B9755EF987Ad6E151",
        //     )
        //     .unwrap()],
        //     backrun_swaps_token_out: vec![H160::from_str(
        //         "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
        //     )
        //     .unwrap()],
        //     backrun_swaps_amount_in: vec![111888798809177],
        //     backrun_swaps_amount_out: vec![567602104693849332],
        // };
    }
}
