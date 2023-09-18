use std::sync::Arc;

use malachite::{num::conversion::traits::RoundingFrom, rounding_modes::RoundingMode, Rational};
use poirot_labeller::Metadata;
use poirot_types::{
    normalized_actions::Actions,
    tree::{GasDetails, TimeTree}
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use reth_primitives::H256;
use tracing::error;

use crate::{ClassifiedMev, Inspector, SpecificMev};

pub struct AtomicBackrunInspector;

impl AtomicBackrunInspector {
    fn process_swaps(
        &self,
        hash: H256,
        metadata: Arc<Metadata>,
        gas_details: &GasDetails,
        swaps: Vec<Vec<Actions>>
    ) -> Vec<(ClassifiedMev, Box<dyn SpecificMev>)> {
        let deltas = self.calculate_swap_deltas(&swaps);

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

        let gas_used = gas_details.gas_paid();
        let (gas_used_usd_appearance, gas_used_usd_finalized) = (
            Rational::from(gas_used) * &metadata.eth_prices.0,
            Rational::from(gas_used) * &metadata.eth_prices.1
        );

        //TODO(WILL): Add fields, see classified_mev.rs
        Some(ClassifiedMev {
            contract: finalized.0,
            gas_details: vec![gas_details.clone()],
            tx_hash: vec![hash],
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

#[async_trait::async_trait]
impl Inspector for AtomicBackrunInspector {
    async fn process_tree(
        &self,
        tree: Arc<TimeTree<Actions>>,
        meta_data: Arc<Metadata>
    ) -> Vec<ClassifiedMev> {
        let intersting_state = tree.inspect_all(|node| {
            node.subactions
                .iter()
                .any(|action| action.is_swap() || action.is_transfer())
        });

        intersting_state
            .into_par_iter()
            .filter_map(|(tx, swaps)| {
                let gas_details = tree.get_gas_details(tx)?;
                self.process_swaps(tx, meta_data.clone(), gas_details, swaps)
            })
            .collect::<Vec<_>>()
    }
}
