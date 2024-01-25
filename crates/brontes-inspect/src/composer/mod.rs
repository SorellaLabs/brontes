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

use std::{collections::HashMap, sync::Arc};

use alloy_primitives::B256;
use brontes_types::classified_mev::Mev;
mod mev_filters;
mod utils;
use async_scoped::{Scope, TokioScope};
use brontes_types::{
    classified_mev::{BundleData, BundleHeader, MevBlock, MevType, PossibleMev},
    db::metadata::MetadataCombined,
    normalized_actions::Actions,
    tree::BlockTree,
};
use mev_filters::{ComposeFunction, MEV_COMPOSABILITY_FILTER, MEV_DEDUPLICATION_FILTER};
use utils::{
    build_mev_header, find_mev_with_matching_tx_hashes, pre_process, sort_mev_by_type,
    BlockPreprocessing,
};

use crate::Inspector;

#[derive(Debug)]
pub struct ComposerResults {
    pub block_details:     MevBlock,
    pub mev_details:       Vec<(BundleHeader, BundleData)>,
    /// all txes with coinbase.transfers that weren't classified
    pub possible_mev_txes: Vec<PossibleMev>,
}

pub async fn compose_mev_results(
    orchestra: &[&Box<dyn Inspector>],
    tree: Arc<BlockTree<Actions>>,
    metadata: Arc<MetadataCombined>,
) -> ComposerResults {
    let pre_processing = pre_process(tree.clone(), metadata.clone());
    let (possible_mev_txes, classified_mev) =
        run_inspectors(orchestra, tree, metadata.clone()).await;

    let possible_arbs = possible_mev_txes.clone();

    let (block_details, mev_details) =
        on_orchestra_resolution(pre_processing, possible_mev_txes, metadata, classified_mev);
    ComposerResults { block_details, mev_details, possible_mev_txes: possible_arbs }
}

async fn run_inspectors(
    orchestra: &[&Box<dyn Inspector>],
    tree: Arc<BlockTree<Actions>>,
    meta_data: Arc<MetadataCombined>,
) -> (Vec<PossibleMev>, Vec<(BundleHeader, BundleData)>) {
    let mut scope: TokioScope<'_, Vec<(BundleHeader, BundleData)>> = unsafe { Scope::create() };
    orchestra
        .iter()
        .for_each(|inspector| scope.spawn(inspector.process_tree(tree.clone(), meta_data.clone())));

    let mut possible_mev_txes = tree
        .tx_roots
        .iter()
        .filter(|r| r.gas_details.coinbase_transfer.is_some() || r.is_private())
        .map(|r| {
            (
                r.tx_hash,
                PossibleMev {
                    tx_hash:           r.tx_hash,
                    position_in_block: r.position,
                    gas_paid:          r.gas_details.gas_paid(),
                },
            )
        })
        .collect::<HashMap<B256, PossibleMev>>();

    // Remove the classified mev txes from the possibly missed tx list
    let results = scope
        .collect()
        .await
        .into_iter()
        .flat_map(|r| r.unwrap())
        .map(|mev| {
            mev.1
                .mev_transaction_hashes()
                .into_iter()
                .for_each(|mev_tx| {
                    possible_mev_txes.remove(&mev_tx);
                });
            mev
        })
        .collect::<Vec<_>>();

    (possible_mev_txes.into_iter().map(|(_, v)| v).collect(), results)
}

fn on_orchestra_resolution(
    pre_processing: BlockPreprocessing,
    possible_mev_txes: Vec<PossibleMev>,
    metadata: Arc<MetadataCombined>,
    orchestra_data: Vec<(BundleHeader, BundleData)>,
) -> (MevBlock, Vec<(BundleHeader, BundleData)>) {
    let mut header =
        build_mev_header(metadata.clone(), &pre_processing, possible_mev_txes, &orchestra_data);

    let mut sorted_mev = sort_mev_by_type(orchestra_data);

    MEV_COMPOSABILITY_FILTER
        .iter()
        .for_each(|(parent_mev_type, compose_fn, child_mev_type)| {
            try_compose_mev(parent_mev_type, child_mev_type, compose_fn, &mut sorted_mev);
        });

    MEV_DEDUPLICATION_FILTER
        .iter()
        .for_each(|(dominant_mev_type, subordinate_mev_type)| {
            deduplicate_mev(dominant_mev_type, subordinate_mev_type, &mut sorted_mev);
        });

    //TODO: (Will) Filter only specific unprofitable types of mev so we can capture
    // bots that are subsidizing their bundles to dry out the competition
    let mut flattened_mev = sorted_mev
        .into_values()
        .flatten()
        .filter(|(classified, _)| {
            if matches!(classified.mev_type, MevType::Sandwich | MevType::Jit | MevType::Backrun) {
                classified.profit_usd > 0.0
            } else {
                true
            }
        })
        .collect::<Vec<_>>();

    let mev_count = flattened_mev.len();
    header.mev_count = mev_count as u64;

    // keep order
    flattened_mev.sort_by(|a, b| a.0.mev_tx_index.cmp(&b.0.mev_tx_index));

    (header, flattened_mev)
}

fn deduplicate_mev(
    dominant_mev_type: &MevType,
    subordinate_mev_types: &[MevType],
    sorted_mev: &mut HashMap<MevType, Vec<(BundleHeader, BundleData)>>,
) {
    let dominant_mev_list = match sorted_mev.get(dominant_mev_type) {
        Some(list) => list,
        None => return,
    };

    let mut removal_indices = Vec::new();

    for (_, dominant_mev_bundle) in dominant_mev_list.iter() {
        let hashes = dominant_mev_bundle.mev_transaction_hashes();

        for &subordinate_mev_type in subordinate_mev_types {
            if let Some(subordinate_mev_list) = sorted_mev.get(&subordinate_mev_type) {
                removal_indices.extend(
                    find_mev_with_matching_tx_hashes(subordinate_mev_list, &hashes)
                        .into_iter()
                        .map(|i| (i, subordinate_mev_type)),
                );
            }
        }
    }

    // Remove the subordinate mev data that is being deduplicated
    for (index, mev_type) in removal_indices.iter().rev() {
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
    parent_mev_type: &MevType,
    child_mev_type: &[MevType],
    compose: &ComposeFunction,
    sorted_mev: &mut HashMap<MevType, Vec<(BundleHeader, BundleData)>>,
) {
    let first_mev_type = child_mev_type[0];
    let mut removal_indices: HashMap<MevType, Vec<usize>> = HashMap::new();

    if let Some(first_mev_list) = sorted_mev.remove(&first_mev_type) {
        for (first_i, (classified, mev_data)) in first_mev_list.iter().enumerate() {
            let tx_hashes = mev_data.mev_transaction_hashes();
            let mut to_compose = vec![(classified.clone(), mev_data.clone())];
            let mut temp_removal_indices = Vec::new();

            for &other_mev_type in child_mev_type.iter().skip(1) {
                if let Some(other_mev_data_list) = sorted_mev.get(&other_mev_type) {
                    let indexes = find_mev_with_matching_tx_hashes(other_mev_data_list, &tx_hashes);
                    if indexes.is_empty() {
                        break
                    }
                    for index in indexes {
                        let (other_classified, other_mev_data) = &other_mev_data_list[index];

                        to_compose.push((other_classified.clone(), other_mev_data.clone()));
                        temp_removal_indices.push((other_mev_type, index));
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
                if !mev_list.is_empty() {
                    mev_list.remove(index);
                }
            }
        }
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
                .with_gas_paid_usd(90.875025)
                .with_expected_profit_usd(13.568977)
                .with_mev_tx_hashes(vec![
                    hex!("22ea36d516f59cc90ccc01042e20f8fba196f32b067a7e5f1510099140ae5e0a").into(),
                    hex!("72eb3269ac013cf663dde9aa11cc3295e0dfb50c7edfcf074c5c57b43611439c").into(),
                    hex!("3b4138bac9dc9fa4e39d8d14c6ecd7ec0144fe26b120ea799317aa15fa35ddcd").into(),
                    hex!("99785f7b76a9347f13591db3574506e9f718060229db2826b4925929ebaea77e").into(),
                    hex!("31dedbae6a8e44ec25f660b3cd0e04524c6476a0431ab610bb4096f82271831b").into(),
                ]);

        inspector_util.run_composer(config, None).await.unwrap();
    }
}
