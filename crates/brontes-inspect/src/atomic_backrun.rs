use std::sync::Arc;

use brontes_database::Metadata;
use brontes_types::{
    classified_mev::{AtomicBackrun, MevType},
    normalized_actions::Actions,
    tree::{GasDetails, TimeTree},
    ToFloatNearest,
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use reth_primitives::{Address, H256};
use tracing::error;

use crate::{shared_utils::SharedInspectorUtils, ClassifiedMev, Inspector, SpecificMev};

#[derive(Default)]
pub struct AtomicBackrunInspector {
    inner: SharedInspectorUtils,
}

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
        let deltas = self.inner.calculate_swap_deltas(&swaps);

        let appearance = self.inner.get_best_usd_delta(
            deltas.clone(),
            metadata.clone(),
            Box::new(|(appearance, _)| appearance),
        )?;

        let finalized = self.inner.get_best_usd_delta(
            deltas,
            metadata.clone(),
            Box::new(|(_, finalized)| finalized),
        )?;

        if finalized.0 != appearance.0 {
            error!("finalized addr != appearance addr");
            return None
        }

        let gas_used = gas_details.gas_paid();
        let (gas_used_usd_appearance, gas_used_usd_finalized) =
            metadata.get_gas_price_usd(gas_used);

        let classified = ClassifiedMev {
            mev_type: MevType::Backrun,
            tx_hash,
            mev_contract,
            block_number: metadata.block_num,
            mev_profit_collector: finalized.0,
            eoa,
            submission_bribe_usd: gas_used_usd_appearance.clone().to_float(),
            finalized_bribe_usd: gas_used_usd_finalized.clone().to_float(),
            finalized_profit_usd: (finalized.1 - gas_used_usd_finalized).to_float(),
            submission_profit_usd: (appearance.1 - gas_used_usd_appearance).to_float(),
        };

        let swaps = swaps.into_iter().flatten().collect::<Vec<_>>();

        let backrun = Box::new(AtomicBackrun {
            tx_hash,
            gas_details,
            swaps_index: swaps
                .iter()
                .map(|s| s.clone().force_swap().index)
                .collect::<Vec<_>>(),
            swaps_from: swaps
                .iter()
                .map(|s| s.clone().force_swap().from)
                .collect::<Vec<_>>(),
            swaps_pool: swaps
                .iter()
                .map(|s| s.clone().force_swap().pool)
                .collect::<Vec<_>>(),
            swaps_token_in: swaps
                .iter()
                .map(|s| s.clone().force_swap().token_in)
                .collect::<Vec<_>>(),
            swaps_token_out: swaps
                .iter()
                .map(|s| s.clone().force_swap().token_out)
                .collect::<Vec<_>>(),
            swaps_amount_in: swaps
                .iter()
                .map(|s| s.clone().force_swap().amount_in.to())
                .collect::<Vec<_>>(),
            swaps_amount_out: swaps
                .iter()
                .map(|s| s.clone().force_swap().amount_out.to())
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
