use std::sync::Arc;

use alloy_primitives::{Address, FixedBytes};
use brontes_types::{
    db::metadata::Metadata,
    mev::{Bundle, Mev, MevBlock, MevCount, MevType, PossibleMevCollection},
    normalized_actions::Actions,
    tree::BlockTree,
    FastHashMap, ToScaledRational, TreeSearchBuilder,
};
use itertools::Itertools;
use malachite::{num::conversion::traits::RoundingFrom, rounding_modes::RoundingMode};
use tracing::log::debug;

pub struct BlockPreprocessing {
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
pub(crate) fn pre_process(tree: Arc<BlockTree<Actions>>) -> BlockPreprocessing {
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
        cumulative_gas_used,
        cumulative_priority_fee,
        total_bribe,
        builder_address,
    }
}

pub(crate) fn build_mev_header(
    metadata: &Arc<Metadata>,
    tree: Arc<BlockTree<Actions>>,
    pre_processing: &BlockPreprocessing,
    possible_mev: PossibleMevCollection,
    mev_count: MevCount,
    orchestra_data: &[Bundle],
) -> MevBlock {
    let (cumulative_mev_priority_fee_paid, cumulative_mev_profit_usd) =
        orchestra_data
            .iter()
            .fold((0u128, 0f64), |(total_fee_paid, total_profit_usd), bundle| {
                let fee_paid = bundle.data.total_priority_fee_paid(
                    tree.header.base_fee_per_gas.unwrap_or_default() as u128,
                );
                let profit_usd = bundle.header.profit_usd;

                (total_fee_paid + fee_paid, total_profit_usd + profit_usd)
            });

    let (builder_eth_profit, builder_mev_profit_usd) = calculate_builder_profit(
        tree,
        metadata,
        pre_processing.cumulative_priority_fee,
        pre_processing.total_bribe,
        orchestra_data,
    );

    let builder_eth_profit = builder_eth_profit.to_scaled_rational(18);

    MevBlock {
        block_hash: metadata.block_hash.into(),
        block_number: metadata.block_num,
        mev_count,
        eth_price: f64::rounding_from(&metadata.eth_prices, RoundingMode::Nearest).0,
        cumulative_gas_used: pre_processing.cumulative_gas_used,
        cumulative_priority_fee: pre_processing.cumulative_priority_fee,
        total_bribe: pre_processing.total_bribe,
        cumulative_mev_priority_fee_paid,
        builder_address: pre_processing.builder_address,
        builder_eth_profit: f64::rounding_from(&builder_eth_profit, RoundingMode::Nearest).0,
        builder_profit_usd: f64::rounding_from(
            &builder_eth_profit * &metadata.eth_prices,
            RoundingMode::Nearest,
        )
        .0,
        builder_mev_profit_usd,
        proposer_fee_recipient: metadata.proposer_fee_recipient,
        proposer_mev_reward: metadata.proposer_mev_reward,
        proposer_profit_usd: metadata.proposer_mev_reward.map(|mev_reward| {
            f64::rounding_from(
                mev_reward.to_scaled_rational(18) * &metadata.eth_prices,
                RoundingMode::Nearest,
            )
            .0
        }),
        cumulative_mev_profit_usd,
        possible_mev,
    }
}

/// Sorts the given MEV data by type.
///
/// This function takes a vector of tuples, where each tuple contains a
/// `BundleHeader` and a `BundleData`. It returns a HashMap where the keys are
/// `MevType` and the values are vectors of tuples (same as input). Each vector
/// contains all the MEVs of the corresponding type.
pub(crate) fn sort_mev_by_type(orchestra_data: Vec<Bundle>) -> FastHashMap<MevType, Vec<Bundle>> {
    orchestra_data
        .into_iter()
        .map(|bundle| (bundle.header.mev_type, bundle))
        .fold(
            FastHashMap::default(),
            |mut acc: FastHashMap<MevType, Vec<Bundle>>, (mev_type, v)| {
                acc.entry(mev_type).or_default().push(v);
                acc
            },
        )
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
    sorted_mev: FastHashMap<MevType, Vec<Bundle>>,
) -> (MevCount, Vec<Bundle>) {
    let mut mev_count = MevCount::default();
    let mut all_filtered_bundles = Vec::new();

    for (mev_type, bundles) in sorted_mev {
        let filtered_bundles: Vec<Bundle> = bundles
            .into_iter()
            .filter(|bundle| {
                if matches!(mev_type, MevType::Sandwich | MevType::Jit | MevType::AtomicArb) {
                    bundle.header.profit_usd > 0.0 || bundle.header.no_pricing_calculated
                } else {
                    true
                }
            })
            .collect();

        // Update  for this MEV type
        let count = filtered_bundles.len() as u64;
        mev_count.bundle_count += count; // Increment total MEV count

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
        MevType::SearcherTx => mev_count.searcher_tx_count = Some(count),
        MevType::Unknown => (),
    }
}

//TODO: Change bundle header to store: revenue, gas & profit
/// Calculate builder profit
///
/// Accounts for ultrasound relay bid adjustments & vertically integrated
/// builder profit
pub fn calculate_builder_profit(
    tree: Arc<BlockTree<Actions>>,
    metadata: &Arc<Metadata>,
    cumulative_priority_fee: u128,
    total_bribe: u128,
    bundles: &[Bundle],
) -> (i128, f64) {
    let builder_address = tree.header.beneficiary;
    let builder_payments: i128 = (cumulative_priority_fee + total_bribe) as i128;

    if metadata.proposer_fee_recipient.is_none() | metadata.proposer_mev_reward.is_none() {
        debug!("Isn't an mev-boost block");
        return (builder_payments, 0.0)
    }

    let builder_sponsorships = tree.clone().collect_all(
        TreeSearchBuilder::default()
            .with_action(Actions::is_eth_transfer)
            .with_from_address(builder_address),
    );

    let builder_sponsorship_amount: i128 = builder_sponsorships
        .flat_map(|(_, v)| v)
        .map(|action| match action {
            Actions::EthTransfer(transfer) => {
                // So we don't double count when deducting proposer mev reward below
                if Some(transfer.to) == metadata.proposer_fee_recipient {
                    0
                } else {
                    transfer.value.to()
                }
            }
            _ => 0,
        })
        .sum::<i128>();

    let builder_info = match metadata.builder_info.as_ref() {
        Some(info) => info,
        None => {
            debug!("Builder info not available, proceeding without it.");
            return (
                builder_payments
                    - builder_sponsorship_amount
                    - metadata.proposer_mev_reward.unwrap_or_default() as i128,
                0.0,
            )
        }
    };
    // Calculate the builder's mev profit from it's associated vertically integrated
    // searchers
    let mev_searching_profit: f64 =
        if builder_info.searchers_eoas.is_empty() && builder_info.searchers_contracts.is_empty() {
            0.0
        } else {
            bundles
                .iter()
                .filter(|bundle| {
                    builder_info.searchers_eoas.contains(&bundle.header.eoa)
                        || bundle
                            .header
                            .mev_contract
                            .map(|mc| builder_info.searchers_contracts.contains(&mc))
                            .unwrap_or(false)
                })
                .map(|bundle| bundle.header.profit_usd)
                .sum()
        };

    let collateral_address = match builder_info.ultrasound_relay_collateral_address {
        Some(address) => address,
        None => {
            // If there's no ultrasound relay collateral address, we don't have to account
            // for collateral address based payments
            debug!("No ultrasound relay collateral address found.");
            return (
                builder_payments
                    - builder_sponsorship_amount
                    - metadata.proposer_mev_reward.unwrap() as i128,
                mev_searching_profit,
            )
        }
    };

    let payment_from_collateral_addr: i128 = tree.tx_roots.last().map_or(0, |root| {
        if root.get_from_address() == collateral_address
            && root.get_to_address() == metadata.block_metadata.proposer_fee_recipient.unwrap()
        {
            match root.get_root_action() {
                Actions::EthTransfer(transfer) => transfer.value.to(), /* Assuming transfer. */
                // value is u128
                _ => 0,
            }
        } else {
            0
        }
    });

    // Calculate final profit considering the sponsorship amount and any collateral
    // payment
    (
        builder_payments - builder_sponsorship_amount - payment_from_collateral_addr,
        mev_searching_profit,
    )
}
