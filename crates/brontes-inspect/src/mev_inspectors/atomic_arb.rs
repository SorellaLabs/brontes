use std::sync::Arc;

use brontes_database::libmdbx::LibmdbxReader;
use brontes_types::{
    constants::{get_stable_type, is_euro_stable, is_gold_stable, is_usd_stable, StableType},
    db::dex::PriceAt,
    mev::{AtomicArb, AtomicArbType, Bundle, BundleData, MevType},
    normalized_actions::{
        accounting::ActionAccounting, Actions, NormalizedAggregator, NormalizedEthTransfer,
        NormalizedFlashLoan, NormalizedSwap, NormalizedTransfer,
    },
    tree::BlockTree,
    ActionIter, FastHashSet, ToFloatNearest, TreeBase, TreeCollector, TreeSearchBuilder, TxInfo,
};
use malachite::{num::basic::traits::Zero, Rational};
use reth_primitives::Address;

use crate::{shared_utils::SharedInspectorUtils, Inspector, Metadata};

pub struct AtomicArbInspector<'db, DB: LibmdbxReader> {
    utils: SharedInspectorUtils<'db, DB>,
}

impl<'db, DB: LibmdbxReader> AtomicArbInspector<'db, DB> {
    pub fn new(quote: Address, db: &'db DB) -> Self {
        Self { utils: SharedInspectorUtils::new(quote, db) }
    }
}

impl<DB: LibmdbxReader> Inspector for AtomicArbInspector<'_, DB> {
    type Result = Vec<Bundle>;

    fn get_id(&self) -> &str {
        "AtomicArb"
    }

    fn process_tree(
        &self,
        tree: Arc<BlockTree<Actions>>,
        meta_data: Arc<Metadata>,
    ) -> Self::Result {
        tree.clone()
            .collect_all(TreeSearchBuilder::default().with_actions([
                Actions::is_swap,
                Actions::is_transfer,
                Actions::is_eth_transfer,
                Actions::is_nested_action,
            ]))
            .t_map(|(k, v)| {
                (
                    k,
                    v.into_iter()
                        .flatten_specified(
                            Actions::try_flash_loan_ref,
                            |actions: NormalizedFlashLoan| {
                                actions
                                    .child_actions
                                    .into_iter()
                                    .chain(actions.repayments.into_iter().map(Actions::from))
                                    .collect::<Vec<_>>()
                            },
                        )
                        .flatten_specified(Actions::try_batch_ref, |batch| {
                            batch
                                .user_swaps
                                .into_iter()
                                .chain(batch.solver_swaps.unwrap_or_default())
                                .map(Into::into)
                                .collect::<Vec<_>>()
                        })
                        .flatten_specified(
                            Actions::try_aggregator_ref,
                            |actions: NormalizedAggregator| {
                                actions.child_actions.into_iter().collect::<Vec<_>>()
                            },
                        )
                        .collect::<Vec<_>>(),
                )
            })
            .t_filter_map(|tree, (tx, actions)| {
                let info = tree.get_tx_info(tx, self.utils.db)?;
                self.process_swaps(
                    info,
                    meta_data.clone(),
                    actions
                        .into_iter()
                        .split_actions::<(Vec<_>, Vec<_>, Vec<_>), _>((
                            Actions::try_swaps_merged,
                            Actions::try_transfer,
                            Actions::try_eth_transfer,
                        )),
                )
            })
            .collect::<Vec<_>>()
    }
}

impl<DB: LibmdbxReader> AtomicArbInspector<'_, DB> {
    fn process_swaps(
        &self,
        info: TxInfo,
        metadata: Arc<Metadata>,
        data: (Vec<NormalizedSwap>, Vec<NormalizedTransfer>, Vec<NormalizedEthTransfer>),
    ) -> Option<Bundle> {
        let (swaps, transfers, eth_transfers) = data;
        let possible_arb_type = self.is_possible_arb(&swaps)?;
        let mev_addresses: FastHashSet<Address> = info.collect_address_set_for_accounting();

        tracing::debug!(?transfers);
        let account_deltas = transfers
            .into_iter()
            .map(Actions::from)
            .chain(eth_transfers.into_iter().map(Actions::from))
            .account_for_actions();

        let (rev, has_dex_price) = if let Some(rev) = self.utils.get_deltas_usd(
            info.tx_index,
            PriceAt::Average,
            &mev_addresses,
            &account_deltas,
            metadata.clone(),
        ) {
            (Some(rev), true)
        } else {
            (
                Some(self.utils.get_available_usd_deltas(
                    info.tx_index,
                    PriceAt::Average,
                    &mev_addresses,
                    &account_deltas,
                    metadata.clone(),
                )),
                false,
            )
        };

        let gas_used = info.gas_details.gas_paid();
        let gas_used_usd = metadata.get_gas_price_usd(gas_used, self.utils.quote);

        let profit = rev
            .map(|rev| rev - &gas_used_usd)
            .filter(|_| has_dex_price)
            .unwrap_or_default();

        let is_profitable = profit > Rational::ZERO;

        let requirement_multiplier = if has_dex_price { 2 } else { 1 };

        let profit = match possible_arb_type {
            AtomicArbType::Triangle => (is_profitable
                || self.process_triangle_arb(&info, requirement_multiplier))
            .then_some(profit),
            AtomicArbType::CrossPair(jump_index) => {
                let stable_arb = is_stable_arb(&swaps, jump_index);
                let cross_or = self.is_cross_pair_or_stable_arb(&info, requirement_multiplier);

                (is_profitable || stable_arb || cross_or).then_some(profit)
            }

            AtomicArbType::StablecoinArb => {
                let cross_or = self.is_cross_pair_or_stable_arb(&info, requirement_multiplier);

                (is_profitable || cross_or).then_some(profit)
            }
            AtomicArbType::LongTail => (self.is_long_tail(&info, requirement_multiplier)
                && is_profitable)
                .then_some(profit),
        }?;

        let backrun = AtomicArb {
            tx_hash: info.tx_hash,
            gas_details: info.gas_details,
            swaps,
            arb_type: possible_arb_type,
        };
        let data = BundleData::AtomicArb(backrun);

        let header = self.utils.build_bundle_header(
            vec![account_deltas],
            vec![info.tx_hash],
            &info,
            profit.to_float(),
            PriceAt::Average,
            &[info.gas_details],
            metadata.clone(),
            MevType::AtomicArb,
            !has_dex_price,
        );
        tracing::debug!("{:#?}", header);

        Some(Bundle { header, data })
    }

    fn is_possible_arb(&self, swaps: &[NormalizedSwap]) -> Option<AtomicArbType> {
        match swaps.len() {
            0 | 1 => None,
            2 => {
                let start = swaps[0].token_in.address;
                let end = swaps[1].token_out.address;
                let is_triangle = start == end;

                let is_continuous = swaps[0].token_out.address == swaps[1].token_in.address;

                if is_triangle && is_continuous {
                    return Some(AtomicArbType::Triangle)
                } else if is_triangle
                    && is_stable_pair(&swaps[0].token_out.symbol, &swaps[1].token_in.symbol)
                {
                    return Some(AtomicArbType::StablecoinArb)
                } else if is_triangle {
                    return Some(AtomicArbType::CrossPair(1))
                } else if is_stable_pair(&swaps[0].token_in.symbol, &swaps[1].token_out.symbol) {
                    return Some(AtomicArbType::StablecoinArb)
                }
                Some(AtomicArbType::LongTail)
            }
            _ => identify_arb_sequence(swaps),
        }
    }

    fn process_triangle_arb(&self, tx_info: &TxInfo, multiplier: u64) -> bool {
        tx_info.is_searcher_of_type_with_count_threshold(MevType::AtomicArb, 20 * multiplier)
            || tx_info.is_labelled_searcher_of_type(MevType::AtomicArb)
            || tx_info.gas_details.coinbase_transfer.is_some() && tx_info.is_private
    }

    fn is_cross_pair_or_stable_arb(&self, tx_info: &TxInfo, multiplier: u64) -> bool {
        tx_info.is_searcher_of_type_with_count_threshold(MevType::AtomicArb, 10 * multiplier)
            || tx_info.is_labelled_searcher_of_type(MevType::AtomicArb)
            || tx_info.is_private
            || tx_info.gas_details.coinbase_transfer.is_some()
    }

    fn is_long_tail(&self, tx_info: &TxInfo, multiplier: u64) -> bool {
        tx_info.is_searcher_of_type_with_count_threshold(MevType::AtomicArb, 10 * multiplier)
            || tx_info.is_labelled_searcher_of_type(MevType::AtomicArb)
            || tx_info.is_private && tx_info.gas_details.coinbase_transfer.is_some()
            || tx_info.mev_contract.is_some()
    }
}

fn identify_arb_sequence(swaps: &[NormalizedSwap]) -> Option<AtomicArbType> {
    let start_token = &swaps.first().unwrap().token_in.symbol;
    let end_token = &swaps.last().unwrap().token_out.symbol;

    let start_address = &swaps.first().unwrap().token_in.address;
    let end_address = &swaps.last().unwrap().token_out.address;

    if start_address != end_address {
        if is_stable_pair(start_token, end_token) {
            return Some(AtomicArbType::StablecoinArb)
        } else {
            return Some(AtomicArbType::LongTail)
        }
    }

    let mut last_out = swaps.first().unwrap().token_out.address;

    for (index, swap) in swaps.iter().skip(1).enumerate() {
        if swap.token_in.address != last_out {
            return Some(AtomicArbType::CrossPair(index + 1))
        }
        last_out = swap.token_out.address;
    }

    Some(AtomicArbType::Triangle)
}

fn is_stable_arb(swaps: &[NormalizedSwap], jump_index: usize) -> bool {
    let token_bought = &swaps[jump_index - 1].token_out.symbol;
    let token_sold = &swaps[jump_index].token_in.symbol;

    is_stable_pair(token_sold, token_bought)
}

fn is_stable_pair(token_in: &str, token_out: &str) -> bool {
    if let Some(stable_type) = get_stable_type(token_in) {
        match stable_type {
            StableType::USD => is_usd_stable(token_out),
            StableType::EURO => is_euro_stable(token_out),
            StableType::GOLD => is_gold_stable(token_out),
        }
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::hex;

    use crate::{
        test_utils::{InspectorTestUtils, InspectorTxRunConfig, USDC_ADDRESS, WETH_ADDRESS},
        Inspectors,
    };

    #[brontes_macros::test]
    async fn test_backrun() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.5).await;

        let tx = hex!("76971a4f00a0a836322c9825b6edf06c8c49bf4261ef86fc88893154283a7124").into();
        let config = InspectorTxRunConfig::new(Inspectors::AtomicArb)
            .with_mev_tx_hashes(vec![tx])
            .with_dex_prices()
            .needs_token(hex!("2559813bbb508c4c79e9ccce4703bcb1f149edd7").into())
            .with_expected_profit_usd(0.188588)
            .with_gas_paid_usd(71.632668);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_not_false_positive_uni_router() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.5).await;
        let tx = hex!("ac1127310fdec0b07e618407eabfb7cdf5ada81dc47e914c76fc759843346a0e").into();
        let config = InspectorTxRunConfig::new(Inspectors::AtomicArb)
            .with_mev_tx_hashes(vec![tx])
            .needs_token(hex!("c18360217d8f7ab5e7c516566761ea12ce7f9d72").into())
            .with_dex_prices();

        inspector_util.assert_no_mev(config).await.unwrap();
    }

    #[brontes_macros::test]
    async fn ensure_proper_calculation() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.5).await;

        let config = InspectorTxRunConfig::new(Inspectors::AtomicArb)
            .with_mev_tx_hashes(vec![hex!(
                "5f9c889b8d6cad5100cc2e6f4a7a59bb53d1cd67f0895320cdb3b25ff43c8fa4"
            )
            .into()])
            .with_dex_prices()
            .needs_tokens(vec![
                WETH_ADDRESS,
                hex!("88e08adb69f2618adf1a3ff6cc43c671612d1ca4").into(),
            ])
            .with_expected_profit_usd(2.63)
            .with_gas_paid_usd(25.3);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn ensure_proper_calculation2() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.5).await;

        let config = InspectorTxRunConfig::new(Inspectors::AtomicArb)
            .with_mev_tx_hashes(vec![hex!(
                "c79494def0565dd49f46c2b7c0221f7eba218ca07638aac3277efe6ab3a2dd66"
            )
            .into()])
            .with_dex_prices()
            .needs_tokens(vec![
                WETH_ADDRESS,
                hex!("88e08adb69f2618adf1a3ff6cc43c671612d1ca4").into(),
            ])
            .with_expected_profit_usd(0.98)
            .with_gas_paid_usd(19.7);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_unix_with_1inch() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.5).await;

        let config = InspectorTxRunConfig::new(Inspectors::AtomicArb)
            .with_mev_tx_hashes(vec![hex!(
                "1cd6862577995835a9e5953845f1d6b5b0462f5762d44319b0e800bcd0c95945"
            )
            .into()])
            .with_dex_prices()
            .needs_tokens(vec![
                WETH_ADDRESS,
                hex!("88e08adb69f2618adf1a3ff6cc43c671612d1ca4").into(),
            ])
            .with_expected_profit_usd(7.47)
            .with_gas_paid_usd(46.59);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_seawise_resolver() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.5).await;

        let config = InspectorTxRunConfig::new(Inspectors::AtomicArb)
            .with_mev_tx_hashes(vec![hex!(
                "2fcc0f54986d594aa7b89ecb475a9b8a62ad9620ab93b7463209b2e7fb58bc1c"
            )
            .into()])
            .with_dex_prices()
            .needs_tokens(vec![
                WETH_ADDRESS,
                hex!("88e08adb69f2618adf1a3ff6cc43c671612d1ca4").into(),
            ])
            .with_expected_profit_usd(243.98)
            .with_gas_paid_usd(41.01);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_reverting_contract() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.5).await;

        let config = InspectorTxRunConfig::new(Inspectors::AtomicArb)
            .with_mev_tx_hashes(vec![hex!(
                "3cfca5f7d00b7f204f6e1bd51e6113094c9fe8abebafd4354e423aca57d93a9b"
            )
            .into()])
            .with_dex_prices()
            .needs_tokens(vec![WETH_ADDRESS])
            .with_expected_profit_usd(4.08)
            .with_gas_paid_usd(155.64);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_more_seawise() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.5).await;

        let config = InspectorTxRunConfig::new(Inspectors::AtomicArb)
            .with_mev_tx_hashes(vec![hex!(
                "e6b38e0eccb86732ea111c793de03fccb1868c3d081e217b5fdccc93ba7f426a"
            )
            .into()])
            .with_dex_prices()
            .needs_tokens(vec![WETH_ADDRESS])
            .with_expected_profit_usd(2.93)
            .with_gas_paid_usd(22.4);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_more_reverting() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.5).await;

        let config = InspectorTxRunConfig::new(Inspectors::AtomicArb)
            .with_mev_tx_hashes(vec![hex!(
                "1256d56742b69cb0a9ba4db53099b1ffa3af4d68fdc7c8da0d2436afcae215d8"
            )
            .into()])
            .with_dex_prices()
            .needs_tokens(vec![WETH_ADDRESS])
            .with_expected_profit_usd(3.87)
            .with_gas_paid_usd(30.7);

        inspector_util.run_inspector(config, None).await.unwrap();
    }
}
