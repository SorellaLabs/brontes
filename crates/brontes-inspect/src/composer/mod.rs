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

use std::sync::Arc;

use brontes_types::{mev::Mev, FastHashMap};
use tracing::{span, Level};

mod mev_filters;
mod utils;
use brontes_types::{
    db::metadata::Metadata,
    mev::{Bundle, MevBlock, MevType, PossibleMevCollection},
    normalized_actions::Actions,
    tree::BlockTree,
};
use mev_filters::{ComposeFunction, MEV_COMPOSABILITY_FILTER, MEV_DEDUPLICATION_FILTER};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use utils::{
    build_mev_header, filter_and_count_bundles, find_mev_with_matching_tx_hashes, sort_mev_by_type,
};

const DISCOVERY_PRIORITY_FEE_MULTIPLIER: f64 = 2.0;

use brontes_types::mev::events::{Action, TuiEvents};
use tokio::sync::mpsc::UnboundedSender;

use crate::{discovery::DiscoveryInspector, Inspector};

#[derive(Debug)]
pub struct ComposerResults {
    pub block_details:     MevBlock,
    pub mev_details:       Vec<Bundle>,
    /// all txes with coinbase.transfers that weren't classified
    pub possible_mev_txes: PossibleMevCollection,
}

pub fn compose_mev_results(
    orchestra: &[&dyn Inspector<Result = Vec<Bundle>>],
    tree: Arc<BlockTree<Actions>>,
    metadata: Arc<Metadata>,
) -> ComposerResults {
    let (possible_mev_txes, classified_mev) =
        run_inspectors(orchestra, tree.clone(), metadata.clone());

    let possible_arbs = possible_mev_txes.clone();

    let (block_details, mev_details) =
        on_orchestra_resolution(tree, possible_mev_txes, metadata, classified_mev);
    ComposerResults { block_details, mev_details, possible_mev_txes: possible_arbs }
}

fn run_inspectors(
    orchestra: &[&dyn Inspector<Result = Vec<Bundle>>],
    tree: Arc<BlockTree<Actions>>,
    metadata: Arc<Metadata>,
) -> (PossibleMevCollection, Vec<Bundle>) {
    let mut possible_mev_txes =
        DiscoveryInspector::new(DISCOVERY_PRIORITY_FEE_MULTIPLIER).find_possible_mev(tree.clone());

    // Remove the classified mev txes from the possibly missed tx list
    let results = orchestra
        .par_iter()
        .flat_map(|inspector| {
            let span = span!(Level::ERROR, "Inspector", inspector = %inspector.get_id(),block=&metadata.block_num);
            span.in_scope(|| {
                inspector.process_tree(tree.clone(), metadata.clone())
            })
        })
        .collect::<Vec<_>>();

    results.iter().for_each(|bundle| {
        bundle
            .data
            .mev_transaction_hashes()
            .into_iter()
            .for_each(|mev_tx| {
                possible_mev_txes.remove(&mev_tx);
            });
    });

    let mut possible_mev_collection =
        PossibleMevCollection(possible_mev_txes.into_values().collect());
    possible_mev_collection
        .0
        .sort_by(|a, b| a.tx_idx.cmp(&b.tx_idx));

    (possible_mev_collection, results)
}

fn on_orchestra_resolution(
    tree: Arc<BlockTree<Actions>>,
    possible_mev_txes: PossibleMevCollection,
    metadata: Arc<Metadata>,
    orchestra_data: Vec<Bundle>,
) -> (MevBlock, Vec<Bundle>) {
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

    let (mev_count, mut filtered_bundles) = filter_and_count_bundles(sorted_mev);

    let header = build_mev_header(&metadata, tree, possible_mev_txes, mev_count, &filtered_bundles);
    // keep order
    filtered_bundles.sort_by(|a, b| a.header.tx_index.cmp(&b.header.tx_index));

    (header, filtered_bundles)
}

fn deduplicate_mev(
    dominant_mev_type: &MevType,
    subordinate_mev_types: &[MevType],
    sorted_mev: &mut FastHashMap<MevType, Vec<Bundle>>,
) {
    let dominant_mev_list = match sorted_mev.get(dominant_mev_type) {
        Some(list) => list,
        None => return,
    };

    let mut removal_indices = Vec::new();

    for dominant_bundle in dominant_mev_list.iter() {
        let hashes = dominant_bundle.data.mev_transaction_hashes();

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
    sorted_mev: &mut FastHashMap<MevType, Vec<Bundle>>,
) {
    let first_mev_type = child_mev_type[0];
    let mut removal_indices: FastHashMap<MevType, Vec<usize>> = FastHashMap::default();

    if let Some(first_mev_list) = sorted_mev.remove(&first_mev_type) {
        for (first_i, bundle) in first_mev_list.iter().enumerate() {
            let tx_hashes = bundle.data.mev_transaction_hashes();
            let mut to_compose = vec![bundle.clone()];
            let mut temp_removal_indices = Vec::new();

            for &other_mev_type in child_mev_type.iter().skip(1) {
                if let Some(other_mev_data_list) = sorted_mev.get(&other_mev_type) {
                    let indexes = find_mev_with_matching_tx_hashes(other_mev_data_list, &tx_hashes);
                    if indexes.is_empty() {
                        break
                    }
                    for index in indexes {
                        let other_bundle = &other_mev_data_list[index];

                        to_compose.push(other_bundle.clone());
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
                if mev_list.len() > index {
                    mev_list.remove(index);
                }
            }
        }
    }
}

#[cfg(test)]
pub mod tests {
    use alloy_primitives::hex;

    use super::*;
    use crate::{
        test_utils::{ComposerRunConfig, InspectorTestUtils, USDC_ADDRESS},
        Inspectors,
    };

    #[brontes_macros::test]
    pub async fn test_jit_sandwich() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.2).await;

        let config = ComposerRunConfig::new(
            vec![Inspectors::Sandwich, Inspectors::Jit],
            MevType::JitSandwich,
        )
        .with_dex_prices()
        .with_gas_paid_usd(90.875025)
        .with_expected_profit_usd(13.568977)
        .needs_tokens(vec![
            hex!("50d1c9771902476076ecfc8b2a83ad6b9355a4c9").into(),
            hex!("b17548c7b510427baac4e267bea62e800b247173").into(),
        ])
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
