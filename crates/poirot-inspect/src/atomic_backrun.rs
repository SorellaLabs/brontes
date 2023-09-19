use std::sync::Arc;

use malachite::{num::conversion::traits::RoundingFrom, rounding_modes::RoundingMode, Rational};
use poirot_labeller::Metadata;
use poirot_types::{
    classified_mev::{AtomicBackrun, MevType},
    normalized_actions::Actions,
    tree::{GasDetails, TimeTree},
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use reth_primitives::{Address, H256};
use tracing::error;

use crate::{ClassifiedMev, Inspector, SpecificMev};

pub struct AtomicBackrunInspector;

impl AtomicBackrunInspector {
    fn process_swaps(
        &self,
        tx_hash: H256,
        eoa: Address,
        mev_contract: Address,
        metadata: Arc<Metadata>,
        gas_details: GasDetails,
        swaps: Vec<Vec<Actions>>,
    ) -> Option<(ClassifiedMev, Box<dyn SpecificMev>)> {
        let deltas = self.calculate_swap_deltas(&swaps);

        let appearance = self.get_best_usd_delta(
            deltas.clone(),
            metadata.clone(),
            Box::new(|(appearance, _)| appearance),
        )?;

        let finalized = self.get_best_usd_delta(
            deltas,
            metadata.clone(),
            Box::new(|(_, finalized)| finalized),
        )?;

        if finalized.0 != appearance.0 {
            error!("finalized addr != appearance addr");
            return None
        }

        let gas_used = gas_details.gas_paid();
        let (gas_used_usd_appearance, gas_used_usd_finalized) = (
            Rational::from(gas_used) * &metadata.eth_prices.0,
            Rational::from(gas_used) * &metadata.eth_prices.1,
        );

        let classified = ClassifiedMev {
            mev_type: MevType::Backrun,
            tx_hash,
            mev_contract,
            block_number: metadata.block_num,
            mev_profit_collector: finalized.0,
            eoa,
            submission_bribe_usd: f64::rounding_from(
                &gas_used_usd_appearance,
                RoundingMode::Nearest,
            )
            .0,
            finalized_bribe_usd: f64::rounding_from(&gas_used_usd_finalized, RoundingMode::Nearest)
                .0,
            finalized_profit_usd: f64::rounding_from(
                finalized.1 - gas_used_usd_finalized,
                RoundingMode::Nearest,
            )
            .0,
            submission_profit_usd: f64::rounding_from(
                appearance.1 - gas_used_usd_appearance,
                RoundingMode::Nearest,
            )
            .0,
        };
        let backrun = Box::new(AtomicBackrun {
            tx_hash,
            gas_details,
            swaps: swaps
                .into_par_iter()
                .flat_map(|m| {
                    m.into_par_iter()
                        .filter_map(|a| if let Actions::Swap(s) = a { Some(s) } else { None })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
        });
        Some((classified, backrun))
    }
}

#[async_trait::async_trait]
impl Inspector for AtomicBackrunInspector {
    async fn process_tree(
        &self,
        tree: Arc<TimeTree<Actions>>,
        meta_data: Arc<Metadata>,
    ) -> Vec<(ClassifiedMev, Box<dyn SpecificMev>)> {
        let intersting_state = tree.inspect_all(|node| {
            node.subactions
                .iter()
                .any(|action| action.is_swap() || action.is_transfer())
        });

        intersting_state
            .into_par_iter()
            .filter_map(|(tx, swaps)| {
                let gas_details = tree.get_gas_details(tx)?.clone();
                let root = tree.get_root(tx)?;

                self.process_swaps(
                    tx,
                    root.head.address,
                    root.head.data.get_too_address(),
                    meta_data.clone(),
                    gas_details,
                    swaps,
                )
            })
            .collect::<Vec<_>>()
    }
}
