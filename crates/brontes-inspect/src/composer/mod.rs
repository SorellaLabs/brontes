//! The `composer` module in `brontes-inspect` specializes in analyzing and
//! processing MEV data. Its primary functions include composing complex MEV
//! types from simpler ones and deduplicating overlapping MEV occurrences.
//!
//! Leveraging the concepts of MEV composability and precedence, this module
//! aims to provide a structured and insightful representation of MEV activities
//! within a block.
//!
//! ## Key Components
//! - `Composer`: A struct that orchestrates specialized inspectors. It waits
//!   for all results and then proceeds to compose and deduplicate MEV data.
//! - `MEV_COMPOSABILITY_FILTER` and `MEV_DEDUPLICATION_FILTER`: These filters,
//!   defined using the `mev_composability` and `define_mev_precedence` macros,
//!   respectively, establish rules for composing multiple MEV types and setting
//!   precedence among them for deduplication.
//! - Utility Functions: A collection of functions designed to assist in the
//!   composition and deduplication processes of MEV data.
//!
//! ## Usage
//! The `Composer` struct is central to this module. It processes a list of
//! `Inspector` futures to extract MEV data, which is then composed and
//! deduplicated based on the rules defined in the `MEV_COMPOSABILITY_FILTER`
//! and `MEV_DEDUPLICATION_FILTER`.
//!
//! ### Example
//! ```ignore
//! let composer = Composer::new(&orchestra, tree, metadata);
//! // Future execution of the composer to process MEV data
//! ```

use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

mod composer_filters;
mod utils;
use async_scoped::{Scope, TokioScope};
use brontes_database::Metadata;
use brontes_types::{
    classified_mev::{ClassifiedMev, MevBlock, MevType, SpecificMev},
    normalized_actions::Actions,
    tree::BlockTree,
};
use composer_filters::{ComposeFunction, MEV_COMPOSABILITY_FILTER, MEV_DEDUPLICATION_FILTER};
use futures::FutureExt;
use utils::{
    build_mev_header, find_mev_with_matching_tx_hashes, pre_process, sort_mev_by_type,
    BlockPreprocessing,
};

use crate::Inspector;

type InspectorFut<'a> =
    Pin<Box<dyn Future<Output = Vec<(ClassifiedMev, Box<dyn SpecificMev>)>> + Send + 'a>>;

pub type ComposerResults = (MevBlock, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>);

pub struct Composer<'a> {
    inspectors_execution: InspectorFut<'a>,
    pre_processing:       BlockPreprocessing,
    metadata:             Arc<Metadata>,
}

impl<'a> Composer<'a> {
    pub fn new(
        orchestra: &'a [&'a Box<dyn Inspector>],
        tree: Arc<BlockTree<Actions>>,
        meta_data: Arc<Metadata>,
    ) -> Self {
        let processing = pre_process(tree.clone(), meta_data.clone());
        let meta_data_clone = meta_data.clone();
        let future = Box::pin(async move {
            let mut scope: TokioScope<'a, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>> =
                unsafe { Scope::create() };
            orchestra.iter().for_each(|inspector| {
                scope.spawn(inspector.process_tree(tree.clone(), meta_data.clone()))
            });

            scope
                .collect()
                .await
                .into_iter()
                .flat_map(|r| r.unwrap())
                .collect::<Vec<_>>()
        })
            as Pin<Box<dyn Future<Output = Vec<(ClassifiedMev, Box<dyn SpecificMev>)>> + 'a>>;

        Self {
            // The rust compiler struggles to prove that the tokio-scope lifetime is the same as
            // the futures lifetime and errors. the transmute is simply casting the
            // lifetime to what it truly is. This is totally safe and will never cause
            // an error
            inspectors_execution: unsafe { std::mem::transmute(future) },
            pre_processing:       processing,
            metadata:             meta_data_clone,
        }
    }

    fn on_orchestra_resolution(
        &mut self,
        orchestra_data: Vec<(ClassifiedMev, Box<dyn SpecificMev>)>,
    ) -> Poll<ComposerResults> {
        let mut header =
            build_mev_header(self.metadata.clone(), &self.pre_processing, &orchestra_data);

        let mut sorted_mev = sort_mev_by_type(orchestra_data);

        MEV_COMPOSABILITY_FILTER.iter().for_each(
            |(parent_mev_type, compose_fn, child_mev_type)| {
                self.try_compose_mev(parent_mev_type, child_mev_type, compose_fn, &mut sorted_mev);
            },
        );

        MEV_DEDUPLICATION_FILTER
            .iter()
            .for_each(|(dominant_mev_type, subordinate_mev_type)| {
                self.deduplicate_mev(dominant_mev_type, subordinate_mev_type, &mut sorted_mev);
            });

        //TODO: (Will) Filter only specific unprofitable types of mev so we can capture
        // bots that are subsidizing their bundles to dry out the competition
        let flattened_mev = sorted_mev
            .into_values()
            .flatten()
            .filter(|(classified, _)| classified.finalized_profit_usd > 0.0)
            .collect::<Vec<_>>();

        // set the mev count now that all merges & reductions have been made
        let mev_count = flattened_mev.len();
        header.mev_count = mev_count as u64;

        // TODO: (Will) Ensure that insertion order is preserved
        Poll::Ready((header, flattened_mev))
    }

    fn deduplicate_mev(
        &mut self,
        dominant_mev_type: &MevType,
        subordinate_mev_types: &[MevType],
        sorted_mev: &mut HashMap<MevType, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>>,
    ) {
        let dominant_mev_list = match sorted_mev.get(dominant_mev_type) {
            Some(list) => list,
            None => return,
        };

        let mut removal_indices: Vec<(MevType, usize)> = Vec::new();

        for (_, dominant_mev_bundle) in dominant_mev_list.iter() {
            let hashes = dominant_mev_bundle.mev_transaction_hashes();

            for &subordinate_mev_type in subordinate_mev_types {
                if let Some(subordinate_mev_list) = sorted_mev.get(&subordinate_mev_type) {
                    if let Some(index) =
                        find_mev_with_matching_tx_hashes(subordinate_mev_list, &hashes)
                    {
                        removal_indices.push((subordinate_mev_type, index));
                    }
                }
            }
        }

        // Remove the subordinate mev data that is being deduplicated
        for (mev_type, index) in removal_indices.iter().rev() {
            if let Some(mev_list) = sorted_mev.get_mut(mev_type) {
                if mev_list.len() > *index {
                    mev_list.remove(*index);
                }
            }
        }
    }

    /// Attempts to compose a new complex MEV occurrence from a list of
    /// MEV types that can be composed together.
    ///
    /// # Functionality:
    ///
    /// The function first checks if there are any MEV of the first type in
    /// `composable_types` in `sorted_mev`. If there are, it iterates over them.
    /// For each MEV, it gets the transaction hashes associated with that MEV
    /// and attempts to find other MEV in `sorted_mev` that have matching
    /// transaction hashes. If it finds matching MEV for all types in
    /// `composable_types`, it uses the `compose` function to create a new MEV
    /// and adds it to `sorted_mev` under `parent_mev_type`. It also records the
    /// indices of the composed MEV in `removal_indices`.
    ///
    /// After attempting to compose MEV for all MEV of the first type in
    /// `composable_types`, it removes all the composed MEV from `sorted_mev`
    /// using the indices stored in `removal_indices`.
    ///
    /// This function does not return any value. Its purpose is to modify
    /// `sorted_mev` by composing new MEV and removing the composed MEV.
    fn try_compose_mev(
        &mut self,
        parent_mev_type: &MevType,
        child_mev_type: &[MevType],
        compose: &ComposeFunction,
        sorted_mev: &mut HashMap<MevType, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>>,
    ) {
        tracing::info!("{:#?}", sorted_mev);
        let first_mev_type = child_mev_type[0];
        let mut removal_indices: HashMap<MevType, Vec<usize>> = HashMap::new();

        if let Some(first_mev_list) = sorted_mev.remove(&first_mev_type) {
            for (first_i, (classified, mev_data)) in first_mev_list.iter().enumerate() {
                let tx_hashes = mev_data.mev_transaction_hashes();
                let mut to_compose = vec![(classified.clone(), mev_data.clone().into_any())];
                let mut temp_removal_indices = Vec::new();

                for &other_mev_type in child_mev_type.iter().skip(1) {
                    if let Some(other_mev_data_list) = sorted_mev.get(&other_mev_type) {
                        match find_mev_with_matching_tx_hashes(other_mev_data_list, &tx_hashes) {
                            Some(index) => {
                                let (other_classified, other_mev_data) =
                                    &other_mev_data_list[index];

                                to_compose.push((
                                    other_classified.clone(),
                                    other_mev_data.clone().into_any(),
                                ));
                                temp_removal_indices.push((other_mev_type, index));
                            }
                            None => break,
                        }
                    } else {
                        break
                    }
                }

                if to_compose.len() == child_mev_type.len() {
                    sorted_mev
                        .entry(*parent_mev_type)
                        .or_default()
                        .push(compose(to_compose));
                    for (mev_type, index) in temp_removal_indices {
                        removal_indices.entry(mev_type).or_default().push(index);
                    }

                    removal_indices
                        .entry(first_mev_type)
                        .or_default()
                        .push(first_i)
                }
            }

            sorted_mev.insert(first_mev_type, first_mev_list);
        }

        // Remove the mev data that was composed from the sorted mev list
        for (mev_type, indices) in removal_indices {
            if let Some(mev_list) = sorted_mev.get_mut(&mev_type) {
                for &index in indices.iter().rev() {
                    mev_list.remove(index);
                }
            }
        }
    }
}

impl Future for Composer<'_> {
    type Output = ComposerResults;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Poll::Ready(calculation) = self.inspectors_execution.poll_unpin(cx) {
            return self.on_orchestra_resolution(calculation)
        }
        Poll::Pending
    }
}

#[cfg(test)]
pub mod tests {
    use alloy_primitives::hex;
    use brontes_types::classified_mev::JitLiquiditySandwich;
    use serial_test::serial;

    use super::*;
    use crate::test_utils::{ComposerRunConfig, InspectorTestUtils, USDC_ADDRESS};

    #[tokio::test]
    #[serial]
    pub async fn test_jit_sandwich() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.2);

        let config =
            ComposerRunConfig::new(vec![MevType::Sandwich, MevType::Jit], MevType::JitSandwich)
                .with_dex_prices()
                .with_expected_gas_used(90.875025)
                .with_expected_profit_usd(13.568977)
                .with_mev_tx_hashes(vec![
                    hex!("22ea36d516f59cc90ccc01042e20f8fba196f32b067a7e5f1510099140ae5e0a").into(),
                    hex!("72eb3269ac013cf663dde9aa11cc3295e0dfb50c7edfcf074c5c57b43611439c").into(),
                    hex!("3b4138bac9dc9fa4e39d8d14c6ecd7ec0144fe26b120ea799317aa15fa35ddcd").into(),
                    hex!("99785f7b76a9347f13591db3574506e9f718060229db2826b4925929ebaea77e").into(),
                    hex!("31dedbae6a8e44ec25f660b3cd0e04524c6476a0431ab610bb4096f82271831b").into(),
                ]);

        inspector_util
            .run_composer::<JitLiquiditySandwich>(config, None)
            .await
            .unwrap();
    }
}
