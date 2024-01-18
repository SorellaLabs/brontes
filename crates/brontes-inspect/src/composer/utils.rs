use std::sync::Arc;

use brontes_database::Metadata;
use brontes_types::{
    classified_mev::{ClassifiedMev, MevBlock, SpecificMev},
    normalized_actions::Actions,
    tree::BlockTree,
    ToScaledRational,
};
use malachite::{num::conversion::traits::RoundingFrom, rounding_modes::RoundingMode};
use reth_primitives::Address;

pub struct BlockPreprocessing {
    meta_data:           Arc<Metadata>,
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
    meta_data: Arc<Metadata>,
) -> BlockPreprocessing {
    let builder_address = tree.header.beneficiary;
    let cumulative_gas_used = tree
        .tx_roots
        .iter()
        .map(|root| root.gas_details.gas_used)
        .sum::<u128>();

    let cumulative_gas_paid = tree
        .tx_roots
        .iter()
        .map(|root| root.gas_details.effective_gas_price * root.gas_details.gas_used)
        .sum::<u128>();

    BlockPreprocessing { meta_data, cumulative_gas_used, cumulative_gas_paid, builder_address }
}

pub(crate) fn build_mev_header(
    metadata: Arc<Metadata>,
    pre_processing: &BlockPreprocessing,
    orchestra_data: &Vec<(ClassifiedMev, Box<dyn SpecificMev>)>,
) -> MevBlock {
    let total_bribe = orchestra_data
        .iter()
        .map(|(_, mev)| mev.bribe())
        .sum::<u128>();

    let cum_mev_priority_fee_paid = orchestra_data
        .iter()
        .map(|(_, mev)| mev.priority_fee_paid())
        .sum::<u128>();

    //TODO: need to check if decimals are correct
    let builder_eth_profit = (total_bribe + pre_processing.cumulative_gas_paid
        - metadata.proposer_mev_reward.unwrap_or_default()) as i128;

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
        builder_eth_profit,
        builder_finalized_profit_usd: f64::rounding_from(
            builder_eth_profit.to_scaled_rational(18) * &pre_processing.meta_data.eth_prices,
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
            (cum_mev_priority_fee_paid + total_bribe).to_scaled_rational(12)
                * &pre_processing.meta_data.eth_prices,
            RoundingMode::Nearest,
        )
        .0,
    }
}
