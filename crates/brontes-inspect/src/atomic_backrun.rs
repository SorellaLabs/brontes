use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use brontes_database::libmdbx::Libmdbx;
use brontes_types::{
    classified_mev::{AtomicBackrun, MevType},
    normalized_actions::Actions,
    tree::{BlockTree, GasDetails},
    ToFloatNearest,
};
use itertools::Itertools;
use malachite::{num::basic::traits::Zero, Rational};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use reth_primitives::{Address, B256};

use crate::{
    shared_utils::SharedInspectorUtils, ClassifiedMev, Inspector, MetadataCombined, SpecificMev,
};

pub struct AtomicBackrunInspector<'db> {
    inner: SharedInspectorUtils<'db>,
}

impl<'db> AtomicBackrunInspector<'db> {
    pub fn new(quote: Address, db: &'db Libmdbx) -> Self {
        Self { inner: SharedInspectorUtils::new(quote, db) }
    }
}

#[async_trait::async_trait]
impl Inspector for AtomicBackrunInspector<'_> {
    async fn process_tree(
        &self,
        tree: Arc<BlockTree<Actions>>,
        meta_data: Arc<MetadataCombined>,
    ) -> Vec<(ClassifiedMev, SpecificMev)> {
        let intersting_state = tree.collect_all(|node| {
            (
                node.data.is_swap() || node.data.is_transfer() || node.data.is_flash_loan(),
                node.subactions.iter().any(|action| {
                    action.is_swap() || action.is_transfer() || node.data.is_flash_loan()
                }),
            )
        });

        intersting_state
            .into_par_iter()
            .filter_map(|(tx, swaps)| {
                let gas_details = tree.get_gas_details(tx)?.clone();
                let root = tree.get_root(tx)?;
                let idx = root.get_block_position();

                self.process_swaps(
                    tx,
                    idx,
                    root.head.address,
                    root.head.data.get_to_address(),
                    meta_data.clone(),
                    gas_details,
                    vec![swaps],
                )
            })
            .collect::<Vec<_>>()
    }
}

impl AtomicBackrunInspector<'_> {
    fn process_swaps(
        &self,
        tx_hash: B256,
        idx: usize,
        eoa: Address,
        mev_contract: Address,
        metadata: Arc<MetadataCombined>,
        gas_details: GasDetails,
        searcher_actions: Vec<Vec<Actions>>,
    ) -> Option<(ClassifiedMev, SpecificMev)> {
        let swaps = searcher_actions
            .iter()
            .flatten()
            .filter(|s| s.is_swap() || s.is_flash_loan())
            .flat_map(|s| match s.clone() {
                Actions::Swap(s) => vec![s],
                Actions::FlashLoan(f) => f
                    .child_actions
                    .into_iter()
                    .filter(|a| a.is_swap())
                    .map(|s| s.force_swap())
                    .collect_vec(),
                _ => vec![],
            })
            .collect_vec();

        tracing::info!(?swaps);
        // check to see if more than 1 swap
        if swaps.len() <= 1 {
            return None
        } else if swaps.len() == 2 {
            let start = swaps[0].token_in;
            let mid = swaps[0].token_out;
            let mid1 = swaps[1].token_in;
            let end = swaps[1].token_out;
            // if not triangular or more than 2 unique tokens, then return.
            // mid != mid1 looks weird. However it is needed as some transactions such as
            // 0x67d9884157d495df4eaf24b0d65aeca38e1b5aeb79200d030e3bb4bd2cbdcf88 swap to a
            // newer token version
            if !(start == end && mid == mid1 || (start != end || mid != mid1)) {
                return None
            }
        }

        // if only 2 swaps, assert either unique tokens

        let deltas = self.inner.calculate_token_deltas(&searcher_actions);

        let addr_usd_deltas =
            self.inner
                .usd_delta_by_address(idx, deltas, metadata.clone(), false)?;

        let mev_profit_collector = self.inner.profit_collectors(&addr_usd_deltas);

        let rev_usd = addr_usd_deltas
            .values()
            .fold(Rational::ZERO, |acc, delta| acc + delta);

        let gas_used = gas_details.gas_paid();
        let gas_used_usd = metadata.get_gas_price_usd(gas_used);

        // Can change this later to check if people are subsidising arbs to kill ops for
        // competitors
        if &rev_usd - &gas_used_usd <= Rational::ZERO {
            return None
        }

        let classified = ClassifiedMev {
            mev_tx_index: idx as u64,
            mev_type: MevType::Backrun,
            tx_hash,
            mev_contract,
            block_number: metadata.block_num,
            mev_profit_collector,
            eoa,
            finalized_bribe_usd: gas_used_usd.clone().to_float(),
            finalized_profit_usd: (rev_usd - gas_used_usd).to_float(),
        };

        let swaps = searcher_actions
            .into_iter()
            .flatten()
            .filter(|actions| actions.is_swap())
            .map(|s| s.force_swap())
            .collect::<Vec<_>>();

        let backrun = AtomicBackrun { tx_hash, gas_details, swaps };

        Some((classified, SpecificMev::AtomicBackrun(backrun)))
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::hex;
    use serial_test::serial;

    use super::*;
    use crate::test_utils::{InspectorTestUtils, InspectorTxRunConfig, USDC_ADDRESS};

    #[tokio::test]
    #[serial]
    async fn test_backrun() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.5);

        let tx = hex!("76971a4f00a0a836322c9825b6edf06c8c49bf4261ef86fc88893154283a7124").into();
        let config = InspectorTxRunConfig::new(MevType::Backrun)
            .with_mev_tx_hashes(vec![tx])
            .with_dex_prices()
            .with_expected_profit_usd(0.188588)
            .with_expected_gas_used(71.632668);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_simple_triangular() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.5);
        let tx = hex!("67d9884157d495df4eaf24b0d65aeca38e1b5aeb79200d030e3bb4bd2cbdcf88").into();
        let config = InspectorTxRunConfig::new(MevType::Backrun)
            .with_mev_tx_hashes(vec![tx])
            .with_dex_prices()
            .with_expected_profit_usd(311.18)
            .with_expected_gas_used(91.51);

        inspector_util.run_inspector(config, None).await.unwrap();
    }
}
