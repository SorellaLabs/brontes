use std::{collections::HashMap, sync::Arc};

use alloy_primitives::FixedBytes;
use brontes_types::{
    classified_mev::{Bundle, Mev, MevBlock, MevType, PossibleMev},
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
    meta_data:               Arc<MetadataCombined>,
    cumulative_gas_used:     u128,
    cumulative_priority_fee: u128,
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
    meta_data: Arc<MetadataCombined>,
) -> BlockPreprocessing {
    let builder_address = tree.header.beneficiary;
    let cumulative_gas_used = tree
        .tx_roots
        .iter()
        .map(|root| root.gas_details.gas_used)
        .sum::<u128>();

    // Sum the priority fee because the base fee is burnt
    let cumulative_priority_fee = tree
        .tx_roots
        .iter()
        .map(|root| root.gas_details.priority_fee)
        .sum::<u128>();

    BlockPreprocessing { meta_data, cumulative_gas_used, cumulative_priority_fee, builder_address }
}

//TODO: Look into calculating the delta of priority fee + coinbase reward vs
// proposer fee paid. This would act as a great proxy for how much mev we missed
pub(crate) fn build_mev_header(
    metadata: Arc<MetadataCombined>,
    pre_processing: &BlockPreprocessing,
    possible_mev: Vec<PossibleMev>,
    orchestra_data: &Vec<Bundle>,
) -> MevBlock {
    let total_bribe = orchestra_data
        .iter()
        .map(|bundle| bundle.data.bribe())
        .sum::<u128>();

    let cum_mev_priority_fee_paid = orchestra_data
        .iter()
        .map(|bundle| bundle.data.priority_fee_paid())
        .sum::<u128>();

    let builder_eth_profit = Rational::from_signeds(
        (total_bribe as i128 + pre_processing.cumulative_priority_fee as i128)
            - (metadata.proposer_mev_reward.unwrap_or_default() as i128),
        10i128.pow(18),
    );

    MevBlock {
        block_hash: pre_processing.meta_data.block_hash.into(),
        block_number: pre_processing.meta_data.block_num,
        mev_count: orchestra_data.len() as u64,
        eth_price: f64::rounding_from(&pre_processing.meta_data.eth_prices, RoundingMode::Nearest)
            .0,
        cumulative_gas_used: pre_processing.cumulative_gas_used,
        cumulative_priority_fee: pre_processing.cumulative_priority_fee,
        total_bribe,
        cumulative_mev_priority_fee_paid: cum_mev_priority_fee_paid,
        builder_address: pre_processing.builder_address,
        builder_eth_profit: f64::rounding_from(&builder_eth_profit, RoundingMode::Nearest).0,
        builder_profit_usd: f64::rounding_from(
            builder_eth_profit * &pre_processing.meta_data.eth_prices,
            RoundingMode::Nearest,
        )
        .0,
        proposer_fee_recipient: pre_processing.meta_data.proposer_fee_recipient,
        proposer_mev_reward: pre_processing.meta_data.proposer_mev_reward,
        proposer_profit_usd: pre_processing
            .meta_data
            .proposer_mev_reward
            .map(|mev_reward| {
                f64::rounding_from(
                    mev_reward.to_scaled_rational(18) * &pre_processing.meta_data.eth_prices,
                    RoundingMode::Nearest,
                )
                .0
            }),
        cumulative_mev_profit_usd: f64::rounding_from(
            (cum_mev_priority_fee_paid + total_bribe).to_scaled_rational(18)
                * &pre_processing.meta_data.eth_prices,
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
