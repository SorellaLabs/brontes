use std::sync::Arc;

use alloy_primitives::{Address, FixedBytes};
use brontes_types::{
    db::{builder::BuilderInfo, metadata::Metadata, traits::LibmdbxReader},
    mev::{Bundle, Mev, MevBlock, MevCount, MevType, PossibleMevCollection},
    normalized_actions::Action,
    tree::BlockTree,
    FastHashMap, GasDetails, ToFloatNearest, ToScaledRational, TreeSearchBuilder,
};
use malachite::{num::conversion::traits::RoundingFrom, rounding_modes::RoundingMode};

use crate::composer::FilterFn;

pub(crate) fn build_mev_header<DB: LibmdbxReader>(
    metadata: &Arc<Metadata>,
    tree: Arc<BlockTree<Action>>,
    possible_mev: PossibleMevCollection,
    mev_count: MevCount,
    orchestra_data: &[Bundle],
    quote_token: Address,
    db: &'static DB,
) -> MevBlock {
    let (total_mev_priority_fee_paid, total_mev_profit_usd, total_mev_bribe) =
        calculate_block_mev_stats(
            orchestra_data,
            tree.header.base_fee_per_gas.unwrap_or_default().into(),
        );

    let eth_price = metadata.get_eth_price(quote_token);

    let pre_processing = pre_process(tree.clone());

    let block_pnl = calculate_builder_profit(tree.clone(), metadata, orchestra_data, &pre_processing);

    let builder_searcher_bribes_usd = f64::rounding_from(
        block_pnl.builder_searcher_tip.to_scaled_rational(18) * &eth_price,
        RoundingMode::Nearest,
    )
    .0;

    let timeboosted_tx_count = tree.clone().tx_roots.iter().filter(|root| root.timeboosted).count() as u64;
    let timeboosted_tx_mev_count = orchestra_data.iter().filter(|bundle| bundle.header.timeboosted).count() as u64;

    let builder_eth_profit = block_pnl.builder_eth_profit.to_scaled_rational(18);

    let proposer_mev_reward = block_pnl.mev_reward;
    let proposer_profit_usd = proposer_mev_reward.map(|mev_reward| {
        f64::rounding_from(mev_reward.to_scaled_rational(18) * &eth_price, RoundingMode::Nearest).0
    });
    let proposer_fee_recipient = block_pnl.proposer_fee_recipient;
    let builder_name = db
        .try_fetch_builder_info(pre_processing.builder_address)
        .unwrap()
        .and_then(|b| b.name);

    MevBlock {
        block_hash: metadata.block_hash.into(),
        block_number: metadata.block_num,
        mev_count,
        eth_price: f64::rounding_from(&eth_price, RoundingMode::Nearest).0,
        total_gas_used: pre_processing.total_gas_used,
        total_priority_fee: pre_processing.total_priority_fee,
        total_bribe: pre_processing.total_bribe,
        total_mev_bribe,
        total_mev_priority_fee_paid,
        builder_address: pre_processing.builder_address,
        builder_name,
        builder_eth_profit: builder_eth_profit.clone().to_float(),
        builder_profit_usd: f64::rounding_from(
            &builder_eth_profit * &eth_price,
            RoundingMode::Nearest,
        )
        .0,
        builder_mev_profit_usd: block_pnl.builder_mev_profit_usd,
        builder_searcher_bribes: block_pnl.builder_searcher_tip,
        builder_searcher_bribes_usd,
        builder_sponsorship_amount: block_pnl.builder_sponsorship as u128,
        ultrasound_bid_adjusted: block_pnl.ultrasound_bid_adjusted,
        proposer_fee_recipient,
        proposer_mev_reward,
        proposer_profit_usd,
        total_mev_profit_usd,
        timeboosted_profit_usd: block_pnl.timeboosted_profit,
        timeboosted_tx_count,
        timeboosted_tx_mev_count,
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
pub(crate) fn find_mev_with_matching_tx_hashes<'a>(
    mev_data_list: &'a [Bundle],
    tx_hashes: &'a [FixedBytes<32>],
) -> impl Iterator<Item = usize> + 'a {
    mev_data_list
        .iter()
        .enumerate()
        .filter_map(move |(index, bundle)| {
            let tx_hashes_in_mev = bundle.data.mev_transaction_hashes();
            tx_hashes_in_mev
                .iter()
                .any(|hash| tx_hashes.contains(hash))
                .then_some(index)
        })
}

/// Finds the index of the first classified mev in the list whose transaction
/// hashes match any of the provided hashes.
pub(crate) fn try_deduping_mev<'a>(
    tree: Arc<BlockTree<Action>>,
    db: Box<dyn LibmdbxReader>,
    dominate: &'a Bundle,
    mev_data_list: &'a [Bundle],
    extra_filter_function: &'a FilterFn,
    tx_hashes: &'a [FixedBytes<32>],
) -> impl Iterator<Item = usize> + 'a {
    let arc = Arc::new(db);
    mev_data_list
        .iter()
        .enumerate()
        .filter_map(move |(index, bundle)| {
            let tx_hashes_in_mev = bundle.data.mev_transaction_hashes();

            let tx_hash_overlap = tx_hashes_in_mev.iter().any(|hash| tx_hashes.contains(hash));
            let extra_args = if let Some(f) = extra_filter_function {
                f(tree.clone(), arc.clone(), [dominate, bundle])
            } else {
                true
            };
            (tx_hash_overlap && extra_args).then_some(index)
        })
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
                if matches!(mev_type, MevType::Sandwich | MevType::AtomicArb) {
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
        MevType::CexDexTrades => mev_count.cex_dex_trade_count = Some(count),
        MevType::CexDexQuotes => mev_count.cex_dex_quote_count = Some(count),
        MevType::JitCexDex => mev_count.jit_cex_dex_count = Some(count),
        MevType::CexDexRfq => mev_count.cex_dex_rfq_count = Some(count),
        MevType::Jit => mev_count.jit_count = Some(count),
        MevType::JitSandwich => mev_count.jit_sandwich_count = Some(count),
        MevType::AtomicArb => mev_count.atomic_backrun_count = Some(count),
        MevType::Liquidation => mev_count.liquidation_count = Some(count),
        MevType::SearcherTx => mev_count.searcher_tx_count = Some(count),
        MevType::Unknown => (),
    }
}

#[derive(Debug)]
pub struct BlockPnL {
    // ETH profit made by the block builder (in wei)
    pub builder_eth_profit:      i128,
    // Amount of ETH paid by the builder to sponsor transactions in the block
    pub builder_sponsorship:     i128,
    // USD profit of the builders searchers
    pub builder_mev_profit_usd:  f64,
    // ETH reward paid to the proposer (in wei)
    pub mev_reward:              Option<u128>,
    // Address of the proposer fee recipient
    pub proposer_fee_recipient:  Option<Address>,
    // Gas & Tips paid to the builder by it's own vertically integrated
    // searchers
    pub builder_searcher_tip:    u128,
    // If the block was bid adjusted using ultrasound's bid adjustment
    pub ultrasound_bid_adjusted: bool,
    // Timeboosted profit
    pub timeboosted_profit:      f64,
}

impl BlockPnL {
    pub fn new(
        builder_eth_profit: i128,
        builder_sponsorship: i128,
        builder_mev_profit_usd: f64,
        mev_reward: Option<u128>,
        proposer_fee_recipient: Option<Address>,
        builder_searcher_tip: u128,
        ultrasound_bid_adjusted: bool,
        timeboosted_profit: f64,
    ) -> Self {
        Self {
            builder_eth_profit,
            builder_sponsorship,
            builder_mev_profit_usd,
            mev_reward,
            proposer_fee_recipient,
            builder_searcher_tip,
            ultrasound_bid_adjusted,
            timeboosted_profit,
        }
    }
}

/// Calculate builder's block PnL
///
/// Accounts for ultrasound relay bid adjustments, builder transaction
/// sponsorship & vertically integrated searcher builder profit
pub fn calculate_builder_profit(
    tree: Arc<BlockTree<Action>>,
    metadata: &Arc<Metadata>,
    bundles: &[Bundle],
    pre_processing: &BlockPreprocessing,
) -> BlockPnL {
    let builder_address = tree.header.beneficiary;
    let builder_payments: i128 =
        (pre_processing.total_priority_fee + pre_processing.total_bribe) as i128;

    let proposer_mev_reward;
    let proposer_fee_recipient;
    let bid_adjusted;
    let mut mev_searching_profit = 0.0;
    let mut vertically_integrated_searcher_tip = 0;

    let timeboosted_profit = bundles
        .iter()
        .filter(|bundle| bundle.header.timeboosted)
        .fold(0.0, |acc, bundle| acc + bundle.header.profit_usd);

    // Calculate the proposer's mev reward & find the proposer fee recipient address
    // If this fails we fallback to the default values queried from the mev-boost
    // relay data api
    if let Some(builder_info) = metadata.builder_info.as_ref() {
        (proposer_mev_reward, proposer_fee_recipient, bid_adjusted) = proposer_payment(
            &tree,
            builder_address,
            builder_info.ultrasound_relay_collateral_address,
            metadata.proposer_fee_recipient,
        )
        .unwrap_or((
            metadata.proposer_mev_reward.unwrap_or_default() as i128,
            metadata.proposer_fee_recipient,
            false,
        ));

        // Calculate the builder's mev profit from it's associated vertically integrated
        // searchers
        (mev_searching_profit, vertically_integrated_searcher_tip) =
            calculate_mev_searching_profit(bundles, builder_info);
    } else {
        (proposer_mev_reward, proposer_fee_recipient, bid_adjusted) =
            proposer_payment(&tree, builder_address, None, metadata.proposer_fee_recipient)
                .unwrap_or((
                    metadata.proposer_mev_reward.unwrap_or_default() as i128,
                    metadata.proposer_fee_recipient,
                    false,
                ));
    }

    let builder_sponsorship_amount = calculate_builder_sponsorship_amount(
        tree.clone(),
        builder_address,
        pre_processing,
        proposer_fee_recipient,
    );

    BlockPnL::new(
        builder_payments - builder_sponsorship_amount - proposer_mev_reward,
        builder_sponsorship_amount,
        mev_searching_profit,
        Some(proposer_mev_reward as u128),
        proposer_fee_recipient,
        vertically_integrated_searcher_tip,
        bid_adjusted,
        timeboosted_profit,
    )
}

fn proposer_payment(
    tree: &Arc<BlockTree<Action>>,
    builder_address: Address,
    collateral_address: Option<Address>,
    proposer_fee_recipient: Option<Address>,
) -> Option<(i128, Option<Address>, bool)> {
    tree.tx_roots.last().and_then(|root| {
        let from_address = root.get_from_address();
        let to_address = root.get_to_address();

        let from_match = from_address == builder_address
            || collateral_address.map_or(false, |addr| from_address == addr);

        let to_match = proposer_fee_recipient.map_or(false, |addr| to_address == addr);

        let is_from_collateral = collateral_address.map_or(false, |addr| from_address == addr);

        if from_match || to_match {
            if let Action::EthTransfer(transfer) = root.get_root_action() {
                return Some((transfer.value.to(), Some(transfer.to), is_from_collateral))
            }
        }
        None
    })
}

/// Accounts for the profit made by the builders vertically integrated searchers
fn calculate_mev_searching_profit(bundles: &[Bundle], builder_info: &BuilderInfo) -> (f64, u128) {
    if builder_info.searchers_eoas.is_empty() && builder_info.searchers_contracts.is_empty() {
        return (0.0, 0)
    }
    bundles
        .iter()
        .filter(|bundle| {
            builder_info.searchers_eoas.contains(&bundle.header.eoa)
                || bundle
                    .header
                    .mev_contract
                    .map(|mev_contract| builder_info.searchers_contracts.contains(&mev_contract))
                    .unwrap_or(false)
        })
        .fold((0.0, 0), |(accumulated_profit, accumulated_gas), bundle| {
            let profit = if bundle.mev_type() != MevType::SearcherTx {
                bundle.header.profit_usd
            } else {
                0.0
            };
            let gas_paid = bundle.data.total_gas_paid();
            (accumulated_profit + profit, accumulated_gas + gas_paid)
        })
}

/// Calculates the amount of gas and tips that the builder sponsored for
/// transactions in the block.
///
/// This function iterates through all ETH transfers sent from the builder's
/// address. It only considers transfers to addresses that paid the builder more
/// in gas and tips over the block than the sponsorship amount they received,
/// because a builder will only sponsor a transaction if it increases their
/// builder balance at the end of the block. If the recipient is the proposer
/// fee recipient, the transfer amount is ignored.
fn calculate_builder_sponsorship_amount(
    tree: Arc<BlockTree<Action>>,
    builder_address: Address,
    pre_processing: &BlockPreprocessing,
    proposer_fee_recipient: Option<Address>,
) -> i128 {
    let builder_sponsorships = tree.clone().collect_all(
        TreeSearchBuilder::default()
            .with_action(Action::is_eth_transfer)
            .with_from_address(builder_address),
    );

    builder_sponsorships
        .flat_map(|(_, v)| v)
        .map(|action| match action {
            Action::EthTransfer(transfer) => {
                if Some(transfer.to) == proposer_fee_recipient {
                    0
                } else if let Some(gas_details) =
                    pre_processing.gas_details_by_address.get(&transfer.to)
                {
                    let total_paid = gas_details.priority_fee
                        + gas_details.coinbase_transfer.unwrap_or_default();
                    if total_paid > transfer.value.to::<u128>() {
                        transfer.value.to()
                    } else {
                        0
                    }
                } else {
                    0
                }
            }
            _ => 0,
        })
        .sum()
}

pub struct BlockPreprocessing {
    total_gas_used:         u128,
    total_priority_fee:     u128,
    total_bribe:            u128,
    builder_address:        Address,
    gas_details_by_address: FastHashMap<Address, GasDetails>,
}

/// Pre-processes the block data for the Builder PNL calculation
pub(crate) fn pre_process(tree: Arc<BlockTree<Action>>) -> BlockPreprocessing {
    let builder_address = tree.header.beneficiary;

    let (gas_details_by_address, total_gas_used, total_priority_fee, total_bribe) =
        tree.tx_roots.iter().fold(
            (FastHashMap::default(), 0u128, 0u128, 0u128),
            |(mut gas_details_by_address, total_gas_used, total_priority_fee, total_bribe),
             root| {
                let address = root.get_from_address();
                let gas_details_item = &root.gas_details;

                gas_details_by_address
                    .entry(address)
                    .and_modify(|existing: &mut GasDetails| existing.merge(gas_details_item))
                    .or_insert_with(|| *gas_details_item);

                let gas_used = gas_details_item.gas_used;
                let priority_fee = gas_details_item.priority_fee;
                let coinbase_transfer = gas_details_item.coinbase_transfer.unwrap_or(0);

                (
                    gas_details_by_address,
                    total_gas_used + gas_used,
                    total_priority_fee + priority_fee * gas_used,
                    total_bribe + coinbase_transfer,
                )
            },
        );

    BlockPreprocessing {
        total_gas_used,
        total_priority_fee,
        total_bribe,
        builder_address,
        gas_details_by_address,
    }
}

/// Calculates the Mev gas & profit stats for the block
///
/// Returns the total priority fee, tips & profit of mev bundles in the block
/// Ignores the profit of SearcherTx bundles as they are not considered MEV.
fn calculate_block_mev_stats(orchestra_data: &[Bundle], base_fee: u128) -> (u128, f64, u128) {
    orchestra_data.iter().fold(
        (0u128, 0.0, 0u128),
        |(total_fee_paid, total_profit_usd, mev_bribe), bundle| {
            let fee_paid = bundle.data.total_priority_fee_paid(base_fee);
            let profit_usd = if bundle.mev_type() != MevType::SearcherTx {
                bundle.header.profit_usd
            } else {
                0.0
            };
            (
                total_fee_paid + fee_paid,
                total_profit_usd + profit_usd,
                mev_bribe + bundle.data.bribe(),
            )
        },
    )
}
