use std::{collections::HashMap, sync::Arc};

use malachite::{num::conversion::traits::RoundingFrom, rounding_modes::RoundingMode, Rational};
use poirot_labeller::Metadata;
use poirot_types::{
    normalized_actions::Actions,
    tree::{GasDetails, TimeTree}
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use reth_primitives::H256;
use tracing::error;

use crate::{ClassifiedMev, Inspector};

pub struct AtomicBackrunInspector {}

impl AtomicBackrunInspector {
    fn process_swaps(
        &self,
        hash: H256,
        priority_fee: u64,
        metadata: Arc<Metadata>,
        gas_details: &GasDetails,
        swaps: Vec<Vec<Actions>>
    ) -> Option<ClassifiedMev> {
        let deltas = self.calculate_swap_deltas(&swaps);

        let appearance_usd_deltas = deltas
            .clone()
            .into_iter()
            .map(|(caller, tokens)| {
                let summed_value = tokens
                    .into_iter()
                    .map(|(address, mut value)| {
                        if let Some(price) = metadata.token_prices.get(&address) {
                            value *= &price.0;
                        }
                        value
                    })
                    .sum::<Rational>();
                (caller, summed_value)
            })
            .max_by(|x, y| x.1.cmp(&y.1));

        let finalized_usd_deltas = deltas
            .clone()
            .into_iter()
            .map(|(caller, tokens)| {
                let summed_value = tokens
                    .into_iter()
                    .map(|(address, mut value)| {
                        if let Some(price) = metadata.token_prices.get(&address) {
                            value *= &price.1;
                        }
                        value
                    })
                    .sum::<Rational>();
                (caller, summed_value)
            })
            .max_by(|x, y| x.1.cmp(&y.1));
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

        Some(ClassifiedMev {
            contract: finalized.0,
            gas_details: gas_details.clone(),
            tx_hash: hash,
            priority_fee,
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
        let intersting_state =
            tree.inspect_all(|node| node.data.is_swap() || node.data.is_transfer());

        intersting_state
            .into_par_iter()
            .filter_map(|(tx, swaps)| {
                let gas_details = tree.get_gas_details(tx)?;
                self.process_swaps(
                    tx,
                    tree.get_priority_fee_for_transaction(tx).unwrap(),
                    meta_data.clone(),
                    gas_details,
                    swaps
                )
            })
            .collect::<Vec<_>>()
    }
}

pub struct AtomicArb {}
