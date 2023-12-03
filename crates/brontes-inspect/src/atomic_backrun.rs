use std::sync::Arc;

use brontes_database::Metadata;
use brontes_types::{
    classified_mev::{AtomicBackrun, MevType},
    normalized_actions::Actions,
    tree::{GasDetails, TimeTree},
    ToFloatNearest,
};
use futures::stream::StreamExt;
use malachite::{num::basic::traits::Zero, Rational};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use reth_primitives::{Address, B256};

use crate::{shared_utils::SharedInspectorUtils, ClassifiedMev, Inspector, SpecificMev};

pub struct AtomicBackrunInspector {
    inner: SharedInspectorUtils,
}

#[async_trait::async_trait]
impl Inspector for AtomicBackrunInspector {
    async fn process_tree(
        &self,
        tree: Arc<TimeTree<Actions>>,
        meta_data: Arc<Metadata>,
    ) -> Vec<(ClassifiedMev, Box<dyn SpecificMev>)> {
        let intersting_state = tree.collect_all(|node| {
            (
                node.data.is_swap() || node.data.is_transfer(),
                node.subactions
                    .iter()
                    .any(|action| action.is_swap() || action.is_transfer()),
            )
        });

        futures::stream::iter(intersting_state)
            .filter_map(|(tx, swaps)| async {
                let gas_details = tree.get_gas_details(tx)?.clone();
                let root = tree.get_root(tx)?;

                self.process_swaps(
                    tx,
                    root.head.address,
                    root.head.data.get_to_address(),
                    meta_data.clone(),
                    gas_details,
                    vec![swaps],
                )
                .await
            })
            .collect::<Vec<_>>()
            .await
    }
}

impl AtomicBackrunInspector {
    pub fn new(rpc_url: &String) -> Self {
        Self { inner: SharedInspectorUtils::new(rpc_url) }
    }

    async fn process_swaps(
        &self,
        tx_hash: B256,
        eoa: Address,
        mev_contract: Address,
        metadata: Arc<Metadata>,
        gas_details: GasDetails,
        swaps: Vec<Vec<Actions>>,
    ) -> Option<(ClassifiedMev, Box<dyn SpecificMev>)> {
        let deltas = self.inner.calculate_swap_deltas(&swaps).await;

        let appearance = self.inner.get_best_usd_deltas(
            deltas.clone(),
            metadata.clone(),
            Box::new(|(appearance, _)| appearance),
        );

        let profit_collectors = appearance.keys().copied().collect();
        let appearance_usd: Rational = appearance.values().sum();

        let finalized = self.inner.get_best_usd_deltas(
            deltas,
            metadata.clone(),
            Box::new(|(_, finalized)| finalized),
        );
        let finalized_usd: Rational = finalized.values().sum();

        if appearance_usd <= Rational::ZERO || finalized_usd <= Rational::ZERO {
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
            mev_profit_collector: profit_collectors,
            eoa,
            submission_bribe_usd: gas_used_usd_appearance.clone().to_float(),
            finalized_bribe_usd: gas_used_usd_finalized.clone().to_float(),
            finalized_profit_usd: (finalized_usd - gas_used_usd_finalized).to_float(),
            submission_profit_usd: (appearance_usd - gas_used_usd_appearance).to_float(),
        };

        let swaps = swaps
            .into_iter()
            .flatten()
            .filter(|actions| actions.is_swap())
            .map(|s| s.force_swap())
            .collect::<Vec<_>>();

        let backrun = Box::new(AtomicBackrun {
            tx_hash,
            gas_details,
            swaps_index: swaps.iter().map(|s| s.index).collect::<Vec<_>>(),
            swaps_from: swaps.iter().map(|s| s.from).collect::<Vec<_>>(),
            swaps_pool: swaps.iter().map(|s| s.pool).collect::<Vec<_>>(),
            swaps_token_in: swaps.iter().map(|s| s.token_in).collect::<Vec<_>>(),
            swaps_token_out: swaps.iter().map(|s| s.token_out).collect::<Vec<_>>(),
            swaps_amount_in: swaps.iter().map(|s| s.amount_in.to()).collect::<Vec<_>>(),
            swaps_amount_out: swaps.iter().map(|s| s.amount_out.to()).collect::<Vec<_>>(),
        });
        Some((classified, backrun))
    }
}

#[cfg(test)]
mod tests {
    use std::{str::FromStr, time::SystemTime};

    use brontes_classifier::Classifier;
    use brontes_core::{init_tracing, test_utils::init_trace_parser};
    use brontes_database::database::Database;
    use brontes_types::test_utils::write_tree_as_json;
    use serial_test::serial;
    use tokio::sync::mpsc::unbounded_channel;

    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_backrun() {
        dotenv::dotenv().ok();
        init_tracing();
        let block_num = 18522278;

        let (tx, _rx) = unbounded_channel();

        let tracer = init_trace_parser(tokio::runtime::Handle::current().clone(), tx);
        let db = Database::default();
        let classifier = Classifier::new();

        let block = tracer.execute_block(block_num).await.unwrap();
        let metadata = db.get_metadata(block_num).await;

        let tx = block.0.clone().into_iter().take(60).collect::<Vec<_>>();
        let tree = Arc::new(classifier.build_tree(tx, block.1, &metadata));

        // write_tree_as_json(&tree, "./tree.json").await;

        let inspector = AtomicBackrunInspector::default();

        let t0 = SystemTime::now();
        let mev = inspector.process_tree(tree.clone(), metadata.into()).await;
        let t1 = SystemTime::now();
        let delta = t1.duration_since(t0).unwrap().as_micros();
        println!("backrun inspector took: {} us", delta);

        // assert!(
        //     mev[0].0.tx_hash
        //         == B256::from_str(
        //
        // "0x80b53e5e9daa6030d024d70a5be237b4b3d5e05d30fdc7330b62c53a5d3537de"
        //         )
        //         .unwrap()
        // );

        println!("{:#?}", mev);
    }
}
