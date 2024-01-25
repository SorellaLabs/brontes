use std::{collections::HashMap, sync::Arc};

use alloy_primitives::FixedBytes;
use brontes_types::{
    classified_mev::{BundleData, BundleHeader, Mev, MevBlock, MevType, PossibleMev},
    db::metadata::MetadataCombined,
    normalized_actions::Actions,
    tree::BlockTree,
    ToScaledRational,
};
use itertools::Itertools;
use malachite::{num::conversion::traits::RoundingFrom, rounding_modes::RoundingMode, Rational};
use reth_primitives::Address;

//TODO: Calculate priority fee & get average so we can flag outliers
pub struct BlockPreprocessing {
    meta_data:           Arc<MetadataCombined>,
    cumulative_gas_used: u128,
    cumulative_gas_paid: u128,
    builder_address:     Address,
}

/// Pre-processes the block data for the Composer.
///
/// This function extracts the builder address from the block tree header,
/// calculates the cumulative gas used and paid by iterating over the
/// transaction roots in the block tree, and packages these results into a
/// `BlockPreprocessing` struct.
pub(crate) fn pre_process(
    tree: Arc<BlockTree<Actions>>,
    meta_data: Arc<MetadataCombined>,
) -> BlockPreprocessing {
    let builder_address = tree.header.beneficiary;
    let cumulative_gas_used = tree
        .tx_roots
        .iter()
        .map(|root| root.gas_details.gas_used)
        .sum::<u128>();
    //TODO: This should only be priority fee as the base fee is burned (calculation
    // to be confirmed)
    let cumulative_gas_paid = tree
        .tx_roots
        .iter()
        .map(|root| root.gas_details.priority_fee)
        .sum::<u128>();

    BlockPreprocessing { meta_data, cumulative_gas_used, cumulative_gas_paid, builder_address }
}

//TODO: Look into calculating the delta of priority fee + coinbase reward vs
// proposer fee paid. This would act as a great proxy for how much mev we missed
pub(crate) fn build_mev_header(
    metadata: Arc<MetadataCombined>,
    pre_processing: &BlockPreprocessing,
    possible_missed_arbs: Vec<PossibleMev>,
    orchestra_data: &Vec<(BundleHeader, BundleData)>,
) -> MevBlock {
    let total_bribe = orchestra_data
        .iter()
        .map(|(_, mev)| mev.bribe())
        .sum::<u128>();

    let cum_mev_priority_fee_paid = orchestra_data
        .iter()
        .map(|(_, mev)| mev.priority_fee_paid())
        .sum::<u128>();

    let builder_eth_profit = Rational::from_signeds(
        (total_bribe as i128 + pre_processing.cumulative_gas_paid as i128)
            - (metadata.proposer_mev_reward.unwrap_or_default() as i128),
        10i128.pow(18),
    );

    MevBlock {
        block_hash: pre_processing.meta_data.block_hash.into(),
        block_number: pre_processing.meta_data.block_num,
        mev_count: orchestra_data.len() as u64,
        finalized_eth_price: f64::rounding_from(
            &pre_processing.meta_data.eth_prices,
            RoundingMode::Nearest,
        )
        .0,
        cumulative_gas_used: pre_processing.cumulative_gas_used,
        cumulative_gas_paid: pre_processing.cumulative_gas_paid,
        total_bribe,
        cumulative_mev_priority_fee_paid: cum_mev_priority_fee_paid,
        builder_address: pre_processing.builder_address,
        builder_eth_profit: f64::rounding_from(&builder_eth_profit, RoundingMode::Nearest).0,
        builder_finalized_profit_usd: f64::rounding_from(
            builder_eth_profit * &pre_processing.meta_data.eth_prices,
            RoundingMode::Nearest,
        )
        .0,
        proposer_fee_recipient: pre_processing.meta_data.proposer_fee_recipient,
        proposer_mev_reward: pre_processing.meta_data.proposer_mev_reward,
        proposer_finalized_profit_usd: pre_processing.meta_data.proposer_mev_reward.map(
            |mev_reward| {
                f64::rounding_from(
                    mev_reward.to_scaled_rational(18) * &pre_processing.meta_data.eth_prices,
                    RoundingMode::Nearest,
                )
                .0
            },
        ),
        cumulative_mev_finalized_profit_usd: f64::rounding_from(
            (cum_mev_priority_fee_paid + total_bribe).to_scaled_rational(18)
                * &pre_processing.meta_data.eth_prices,
            RoundingMode::Nearest,
        )
        .0,
        possible_missed_arbs,
    }
}

/// Sorts the given MEV data by type.
///
/// This function takes a vector of tuples, where each tuple contains a
/// `BundleHeader` and a `BundleData`. It returns a HashMap where the keys are
/// `MevType` and the values are vectors of tuples (same as input). Each vector
/// contains all the MEVs of the corresponding type.
pub(crate) fn sort_mev_by_type(
    orchestra_data: Vec<(BundleHeader, BundleData)>,
) -> HashMap<MevType, Vec<(BundleHeader, BundleData)>> {
    orchestra_data
        .into_iter()
        .map(|(classified_mev, specific)| (classified_mev.mev_type, (classified_mev, specific)))
        .fold(
            HashMap::default(),
            |mut acc: HashMap<MevType, Vec<(BundleHeader, BundleData)>>, (mev_type, v)| {
                acc.entry(mev_type).or_default().push(v);
                acc
            },
        )
}

/// Finds the index of the first classified mev in the list whose transaction
/// hashes match any of the provided hashes.
pub(crate) fn find_mev_with_matching_tx_hashes(
    mev_data_list: &[(BundleHeader, BundleData)],
    tx_hashes: &[FixedBytes<32>],
) -> Vec<usize> {
    mev_data_list
        .iter()
        .enumerate()
        .filter_map(|(index, (_, mev_data))| {
            let tx_hashes_in_mev = mev_data.mev_transaction_hashes();
            if tx_hashes_in_mev.iter().any(|hash| tx_hashes.contains(hash)) {
                Some(index)
            } else {
                None
            }
        })
        .collect_vec()
}
