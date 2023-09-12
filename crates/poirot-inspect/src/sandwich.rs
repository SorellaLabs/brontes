use std::sync::Arc;

use malachite::{num::conversion::traits::RoundingFrom, rounding_modes::RoundingMode, Rational};
use poirot_labeller::Metadata;
use poirot_types::{
    normalized_actions::Actions,
    tree::{GasDetails, Node, TimeTree}
};
use reth_primitives::H256;
use tracing::error;

use crate::{ClassifiedMev, Inspector};

pub struct SandwichInspector;

#[async_trait::async_trait]
impl Inspector for SandwichInspector {
    async fn process_tree(
        &self,
        tree: Arc<TimeTree<Actions>>,
        meta_data: Arc<Metadata>
    ) -> Vec<ClassifiedMev> {
        // lets grab the set of all possible sandwich txes
        let mut iter = tree.roots.iter();
        if iter.len() < 3 {
            return vec![]
        }

        let mut set = Vec::new();
        let mut pairs = (iter.next().unwrap(), iter.next().unwrap());

        for root in iter {
            if root.head.address == pairs.0.head.address {
                set.push((root.tx_hash, pairs.0.tx_hash));
            }
            pairs.0 = pairs.1;
            pairs.1 = root;
        }

        let search_fn = |node: &Node<Actions>| {
            node.subactions
                .iter()
                .any(|action| action.is_swap() || action.is_transfer())
        };

        set.into_iter()
            .filter_map(|(tx0, tx1)| {
                let priority = [
                    tree.get_priority_fee_for_transaction(tx0).unwrap(),
                    tree.get_priority_fee_for_transaction(tx1).unwrap()
                ];

                let gas = [
                    tree.get_gas_details(tx0).cloned().unwrap(),
                    tree.get_gas_details(tx1).cloned().unwrap()
                ];

                self.calculate_sandwich(
                    meta_data.clone(),
                    [tx0, tx1],
                    gas,
                    priority,
                    vec![tree.inspect(tx0, search_fn.clone()), tree.inspect(tx1, search_fn)]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<Vec<Actions>>>()
                )
            })
            .collect::<Vec<_>>()
    }
}

impl SandwichInspector {
    fn calculate_sandwich(
        &self,
        metadata: Arc<Metadata>,
        txes: [H256; 2],
        gas_details: [GasDetails; 2],
        priority_fee: [u64; 2],
        actions: Vec<Vec<Actions>>
    ) -> Option<ClassifiedMev> {
        let deltas = self.calculate_swap_deltas(&actions);

        let appearance_usd_deltas = self.get_best_usd_delta(
            deltas.clone(),
            metadata.clone(),
            Box::new(|(appearance, _)| appearance)
        );

        let finalized_usd_deltas =
            self.get_best_usd_delta(deltas, metadata.clone(), Box::new(|(_, finalized)| finalized));

        if finalized_usd_deltas.is_none() || appearance_usd_deltas.is_none() {
            return None
        }
        let (finalized, appearance) =
            (finalized_usd_deltas.unwrap(), appearance_usd_deltas.unwrap());

        if finalized.0 != appearance.0 {
            error!("finalized addr != appearance addr");
            return None
        }

        let gas_used = gas_details.iter().map(|g| g.gas_paid()).sum::<u64>();

        let (gas_used_usd_appearance, gas_used_usd_finalized) = (
            Rational::from(gas_used) * &metadata.eth_prices.0,
            Rational::from(gas_used) * &metadata.eth_prices.1
        );

        Some(ClassifiedMev {
            contract: finalized.0,
            gas_details: gas_details.to_vec(),
            tx_hash: txes.to_vec(),
            priority_fee: priority_fee.to_vec(),
            block_finalized_profit_usd: f64::rounding_from(
                &finalized.1 - gas_used_usd_finalized,
                RoundingMode::Nearest
            )
            .0,
            block_appearance_profit_usd: f64::rounding_from(
                &appearance.1 - gas_used_usd_appearance,
                RoundingMode::Nearest
            )
            .0,
            block_finalized_revenue_usd: f64::rounding_from(finalized.1, RoundingMode::Nearest).0,
            block_appearance_revenue_usd: f64::rounding_from(appearance.1, RoundingMode::Nearest).0
        })
    }
}
