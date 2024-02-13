use std::{collections::HashMap, sync::Arc};

use alloy_primitives::{Address, FixedBytes};
use brontes_types::{
    db::metadata::Metadata,
    mev::{Bundle, Mev, MevBlock, MevCount, MevType, PossibleMevCollection},
    normalized_actions::Actions,
    tree::BlockTree,
    ToScaledRational, TreeSearchArgs,
};
use itertools::Itertools;
use malachite::{num::conversion::traits::RoundingFrom, rounding_modes::RoundingMode};

//TODO: Calculate priority fee & get average so we can flag outliers
pub struct BlockPreprocessing {
    metadata:                Arc<Metadata>,
    cumulative_gas_used:     u128,
    cumulative_priority_fee: u128,
    total_bribe:             u128,
    builder_address:         Address,
}

/// Pre-processes the block data for the Composer.
///
/// This function extracts the builder address from the block tree header,
/// calculates the cumulative gas used and paid by iterating over the
/// transaction roots in the block tree, and packages these results into a
/// `BlockPreprocessing` struct.
pub(crate) fn pre_process(
    tree: Arc<BlockTree<Actions>>,
    metadata: Arc<Metadata>,
) -> BlockPreprocessing {
    let builder_address = tree.header.beneficiary;

    let (cumulative_gas_used, cumulative_priority_fee, total_bribe) = tree.tx_roots.iter().fold(
        (0u128, 0u128, 0u128),
        |(cumulative_gas_used, cumulative_priority_fee, total_bribe), root| {
            let gas_details = &root.gas_details;

            let gas_used = gas_details.gas_used;
            let priority_fee = gas_details.priority_fee;
            let bribe = gas_details.coinbase_transfer();

            (
                cumulative_gas_used + gas_used,
                cumulative_priority_fee + priority_fee * gas_used,
                total_bribe + bribe,
            )
        },
    );

    BlockPreprocessing {
        metadata,
        cumulative_gas_used,
        cumulative_priority_fee,
        total_bribe,
        builder_address,
    }
}

//TODO: Clean up & fix
pub(crate) fn build_mev_header(
    metadata: Arc<Metadata>,
    tree: Arc<BlockTree<Actions>>,
    pre_processing: &BlockPreprocessing,
    possible_mev: PossibleMevCollection,
    orchestra_data: &[Bundle],
) -> MevBlock {
    let cum_mev_priority_fee_paid = orchestra_data
        .iter()
        .map(|bundle| {
            bundle
                .data
                .total_priority_fee_paid(tree.header.base_fee_per_gas.unwrap_or_default() as u128)
        })
        .sum();
    let builder_eth_profit = calculate_builder_profit(tree, metadata)
        .unwrap()
        .to_scaled_rational(18);

    MevBlock {
        block_hash: pre_processing.metadata.block_hash.into(),
        block_number: pre_processing.metadata.block_num,
        mev_count: MevCount::default(),
        eth_price: f64::rounding_from(&pre_processing.metadata.eth_prices, RoundingMode::Nearest).0,
        cumulative_gas_used: pre_processing.cumulative_gas_used,
        cumulative_priority_fee: pre_processing.cumulative_priority_fee,
        total_bribe: pre_processing.total_bribe,
        cumulative_mev_priority_fee_paid: cum_mev_priority_fee_paid,
        builder_address: pre_processing.builder_address,
        builder_eth_profit: f64::rounding_from(&builder_eth_profit, RoundingMode::Nearest).0,
        builder_profit_usd: f64::rounding_from(
            builder_eth_profit * &pre_processing.metadata.eth_prices,
            RoundingMode::Nearest,
        )
        .0,
        proposer_fee_recipient: pre_processing.metadata.proposer_fee_recipient,
        proposer_mev_reward: pre_processing.metadata.proposer_mev_reward,
        proposer_profit_usd: pre_processing
            .metadata
            .proposer_mev_reward
            .map(|mev_reward| {
                f64::rounding_from(
                    mev_reward.to_scaled_rational(18) * &pre_processing.metadata.eth_prices,
                    RoundingMode::Nearest,
                )
                .0
            }),
        //TODO: This is wron need to fix
        cumulative_mev_profit_usd: f64::rounding_from(
            (cum_mev_priority_fee_paid + pre_processing.total_bribe).to_scaled_rational(18)
                * &pre_processing.metadata.eth_prices,
            RoundingMode::Nearest,
        )
        .0,
        possible_mev,
    }
}

/// Sorts the given MEV data by type.
///
/// This function takes a vector of tuples, where each tuple contains a
/// `BundleHeader` and a `BundleData`. It returns a HashMap where the keys are
/// `MevType` and the values are vectors of tuples (same as input). Each vector
/// contains all the MEVs of the corresponding type.
pub(crate) fn sort_mev_by_type(orchestra_data: Vec<Bundle>) -> HashMap<MevType, Vec<Bundle>> {
    orchestra_data
        .into_iter()
        .map(|bundle| (bundle.header.mev_type, bundle))
        .fold(HashMap::default(), |mut acc: HashMap<MevType, Vec<Bundle>>, (mev_type, v)| {
            acc.entry(mev_type).or_default().push(v);
            acc
        })
}

/// Finds the index of the first classified mev in the list whose transaction
/// hashes match any of the provided hashes.
pub(crate) fn find_mev_with_matching_tx_hashes(
    mev_data_list: &[Bundle],
    tx_hashes: &[FixedBytes<32>],
) -> Vec<usize> {
    mev_data_list
        .iter()
        .enumerate()
        .filter_map(|(index, bundle)| {
            let tx_hashes_in_mev = bundle.data.mev_transaction_hashes();
            if tx_hashes_in_mev.iter().any(|hash| tx_hashes.contains(hash)) {
                Some(index)
            } else {
                None
            }
        })
        .collect_vec()
}

pub fn filter_and_count_bundles(
    sorted_mev: HashMap<MevType, Vec<Bundle>>,
) -> (MevCount, Vec<Bundle>) {
    let mut mev_count = MevCount::default();
    let mut all_filtered_bundles = Vec::new();

    for (mev_type, bundles) in sorted_mev {
        let filtered_bundles: Vec<Bundle> = bundles
            .into_iter()
            .filter(|bundle| {
                if matches!(mev_type, MevType::Sandwich | MevType::Jit | MevType::AtomicArb) {
                    bundle.header.profit_usd > 0.0
                } else {
                    true
                }
            })
            .collect();

        // Update count for this MEV type
        let count = filtered_bundles.len() as u64;
        mev_count.mev_count += count; // Increment total MEV count

        if count != 0 {
            update_mev_count(&mut mev_count, mev_type, count);
        }

        // Add the filtered bundles to the overall list
        all_filtered_bundles.extend(filtered_bundles);
    }

    (mev_count, all_filtered_bundles)
}

fn update_mev_count(mev_count: &mut MevCount, mev_type: MevType, count: u64) {
    match mev_type {
        MevType::Sandwich => mev_count.sandwich_count = Some(count),
        MevType::CexDex => mev_count.cex_dex_count = Some(count),
        MevType::Jit => mev_count.jit_count = Some(count),
        MevType::JitSandwich => mev_count.jit_sandwich_count = Some(count),
        MevType::AtomicArb => mev_count.atomic_backrun_count = Some(count),
        MevType::Liquidation => mev_count.liquidation_count = Some(count),
        MevType::Unknown => (),
    }
}

pub fn calculate_builder_profit(
    tree: Arc<BlockTree<Actions>>,
    metadata: Arc<Metadata>,
) -> eyre::Result<u128> {
    let coinbase_transfers = tree
        .tx_roots
        .iter()
        .filter_map(|root| root.gas_details.coinbase_transfer)
        .sum::<u128>(); // Specify the type of sum

    let builder_collateral_amount = tree
        .collect_all(|node, data| TreeSearchArgs {
            collect_current_node:  data.get_ref(node.data).map(|a| a.get_from_address())
                == metadata
                    .builder_info
                    .as_ref()
                    .and_then(|b| b.ultrasound_relay_collateral_address)
                && data
                    .get_ref(node.data)
                    .map(|d| d.is_eth_transfer())
                    .unwrap_or_default(),
            child_node_to_collect: node
                .get_all_sub_actions()
                .iter()
                .filter_map(|n| data.get_ref(*n))
                .any(|sub_node| {
                    Some(sub_node.get_from_address())
                        == metadata
                            .builder_info
                            .as_ref()
                            .and_then(|b| b.ultrasound_relay_collateral_address)
                        && sub_node.is_eth_transfer()
                }),
        })
        .iter()
        .flat_map(|(_fixed_bytes, actions)| {
            actions.iter().filter_map(|action| {
                let Actions::EthTransfer(transfer) = action else { return None };
                Some(transfer.value.to::<u128>())
            })
        })
        .sum::<u128>();

    Ok(coinbase_transfers - builder_collateral_amount)
}
