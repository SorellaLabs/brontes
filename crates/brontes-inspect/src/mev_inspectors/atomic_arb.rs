use std::sync::Arc;

use brontes_database::libmdbx::LibmdbxReader;
use brontes_metrics::inspectors::{OutlierMetrics, ProfitMetrics};
use brontes_types::{
    constants::{get_stable_type, is_euro_stable, is_gold_stable, is_usd_stable, StableType},
    db::dex::PriceAt,
    mev::{AtomicArb, AtomicArbType, Bundle, BundleData, MevType},
    normalized_actions::{
        accounting::ActionAccounting, Action, NormalizedEthTransfer, NormalizedSwap,
        NormalizedTransfer,
    },
    BlockData, FastHashSet, IntoZip, MultiBlockData, ToFloatNearest, TreeBase, TreeCollector,
    TreeSearchBuilder, TxInfo,
};
use itertools::Itertools;
use malachite::{num::basic::traits::Zero, Rational};
use reth_primitives::{Address, B256};

use crate::{
    shared_utils::SharedInspectorUtils, BlockTree, Inspector, Metadata, MAX_PROFIT, MIN_PROFIT,
};

const MAX_PRICE_DIFF: Rational = Rational::const_from_unsigneds(99, 100);

// figure out why
pub struct AtomicArbInspector<'db, DB: LibmdbxReader> {
    utils: SharedInspectorUtils<'db, DB>,
}

impl<'db, DB: LibmdbxReader> AtomicArbInspector<'db, DB> {
    pub fn new(
        quote: Address,
        db: &'db DB,
        metrics: Option<OutlierMetrics>,
        profit_metrics: Option<ProfitMetrics>,
    ) -> Self {
        Self { utils: SharedInspectorUtils::new(quote, db, metrics, profit_metrics) }
    }
}

impl<DB: LibmdbxReader> Inspector for AtomicArbInspector<'_, DB> {
    type Result = Vec<Bundle>;

    // we use a 2 block window so that we can always have a trigger tx
    fn block_window(&self) -> usize {
        2
    }

    fn get_id(&self) -> &str {
        "AtomicArb"
    }

    fn get_quote_token(&self) -> Address {
        self.utils.quote
    }

    fn inspect_block(&self, data: MultiBlockData) -> Self::Result {
        let BlockData { metadata, tree } = data.get_most_recent_block();

        let execution = || {
            tree.clone()
                .collect_all(TreeSearchBuilder::default().with_actions([
                    Action::is_swap,
                    Action::is_transfer,
                    Action::is_eth_transfer,
                    Action::is_nested_action,
                ]))
                .t_full_map(|(tree, v)| {
                    let (tx_hashes, v): (Vec<_>, Vec<_>) = v.unzip();
                    (
                        tree.get_tx_info_batch(&tx_hashes, self.utils.db),
                        v.into_iter().map(|v| {
                            self.utils
                                .flatten_nested_actions_default(v.into_iter())
                                .collect::<Vec<_>>()
                        }),
                    )
                })
                .into_zip()
                .filter_map(|(info, action)| {
                    let info = info??;
                    let actions = action?;

                    self.process_swaps(
                        data.per_block_data
                            .iter()
                            .map(|inner| inner.tree.clone())
                            .collect_vec(),
                        info,
                        metadata.clone(),
                        actions
                            .into_iter()
                            .split_actions::<(Vec<_>, Vec<_>, Vec<_>), _>((
                                Action::try_swaps_merged,
                                Action::try_transfer,
                                Action::try_eth_transfer,
                            )),
                    )
                })
                .collect::<Vec<_>>()
        };

        self.utils
            .get_metrics()
            .map(|m| m.run_inspector(MevType::AtomicArb, execution))
            .unwrap_or_else(&execution)
    }
}

impl<DB: LibmdbxReader> AtomicArbInspector<'_, DB> {
    fn process_swaps(
        &self,
        trees: Vec<Arc<BlockTree<Action>>>,
        info: TxInfo,
        metadata: Arc<Metadata>,
        data: (Vec<NormalizedSwap>, Vec<NormalizedTransfer>, Vec<NormalizedEthTransfer>),
    ) -> Option<Bundle> {
        tracing::trace!(?info, "trying atomic");
        let (mut swaps, transfers, eth_transfers) = data;
        let mev_addresses: FastHashSet<Address> = info.collect_address_set_for_accounting();

        let mut ignore_addresses = mev_addresses.clone();

        swaps.iter().for_each(|s| {
            ignore_addresses.insert(s.pool);
        });

        swaps.extend(self.utils.try_create_swaps(&transfers, ignore_addresses));

        let possible_arb_type = self.is_possible_arb(&swaps)?;

        let account_deltas = transfers
            .into_iter()
            .map(Action::from)
            .chain(eth_transfers.into_iter().map(Action::from))
            .chain(info.get_total_eth_value().iter().cloned().map(Action::from))
            .account_for_actions();

        let mut has_dex_price = self.utils.valid_pricing(
            metadata.clone(),
            &swaps,
            account_deltas
                .values()
                .flat_map(|k| {
                    k.iter()
                        .filter(|(_, v)| *v != &Rational::ZERO)
                        .map(|(k, _)| k)
                })
                .unique(),
            info.tx_index as usize,
            MAX_PRICE_DIFF,
            MevType::AtomicArb,
        );

        let gas_used = info.gas_details.gas_paid();
        let gas_used_usd = metadata.get_gas_price_usd(gas_used, self.utils.quote);

        let rev = if let Some(rev) = self.utils.get_deltas_usd(
            info.tx_index,
            PriceAt::Average,
            &mev_addresses,
            &account_deltas,
            metadata.clone(),
            false,
        ) {
            Some(rev)
        } else {
            has_dex_price = false;
            Some(Rational::ZERO)
        };

        let mut profit = rev
            .map(|rev| rev - &gas_used_usd)
            .filter(|_| has_dex_price)
            .unwrap_or_default();

        if profit >= MAX_PROFIT || profit <= MIN_PROFIT {
            has_dex_price = false;
            profit = Rational::ZERO;
        }

        let is_profitable = profit > Rational::ZERO;

        let requirement_multiplier = if has_dex_price { 1 } else { 2 };

        let profit = match possible_arb_type {
            AtomicArbType::Triangle => (is_profitable
                || self.process_triangle_arb(&info, requirement_multiplier))
            .then_some(profit),
            AtomicArbType::CrossPair(jump_index) => (is_profitable
                || self.is_stable_arb(&swaps, jump_index)
                || self.is_cross_pair_or_stable_arb(&info, requirement_multiplier))
            .then_some(profit),

            AtomicArbType::StablecoinArb => (is_profitable
                || self.is_cross_pair_or_stable_arb(&info, requirement_multiplier))
            .then_some(profit),
            AtomicArbType::LongTail => (self.is_long_tail(&info, requirement_multiplier)
                && is_profitable
                || self.is_long_tail(&info, requirement_multiplier) & !has_dex_price)
                .then_some(profit),
        }?;

        // given we have a atomic arb now, we will go and try to find the trigger
        // transaction that lead to this arb.

        let protocols = self.utils.get_related_protocols_atomic(&trees);
        let trigger_tx = self.find_trigger_tx(&info, trees, &swaps);
        let profit_usd = profit.to_float();
        let protocols_str = protocols.iter().map(|p| p.to_string()).collect_vec();

        tracing::debug!(?protocols, ?profit_usd, ?info.tx_hash, "Found atomic arb");

        let backrun = AtomicArb {
            block_number: metadata.block_num,
            trigger_tx,
            tx_hash: info.tx_hash,
            gas_details: info.gas_details,
            swaps,
            arb_type: possible_arb_type,
            profit_usd,
            protocols: protocols_str,
        };
        let data = BundleData::AtomicArb(backrun);
        let header = self.utils.build_bundle_header(
            vec![account_deltas],
            vec![info.tx_hash],
            &info,
            profit_usd,
            &[info.gas_details],
            metadata.clone(),
            MevType::AtomicArb,
            !has_dex_price,
            |this, token, amount| {
                this.get_token_value_dex(
                    info.tx_index as usize,
                    PriceAt::Average,
                    token,
                    &amount,
                    &metadata,
                )
            },
        );

        if profit_usd.abs() > 100.0 {
            tracing::warn!(?header.tx_hash, ?profit_usd, "abnormal profit");
        }



        self.utils.get_profit_metrics().inspect(|m| {
            if possible_arb_type != AtomicArbType::LongTail {
                m.publish_profit_metrics(MevType::AtomicArb, protocols, profit_usd)
            } 
        });

        Some(Bundle { header, data })
    }

    /// goes back through the tree until it finds a transaction that occurred
    /// before the atomic arb that use the same liquidity pool for a swap.
    fn find_trigger_tx(
        &self,
        arb_info: &TxInfo,
        mut trees: Vec<Arc<BlockTree<Action>>>,
        swaps: &[NormalizedSwap],
    ) -> B256 {
        let this_tree = trees.pop().unwrap();

        trees
            .into_iter()
            .flat_map(|tree| tree.tx_roots.clone().into_iter().rev().collect_vec())
            .chain(
                this_tree
                    .tx_roots
                    .clone()
                    .into_iter()
                    .take(arb_info.tx_index as usize)
                    .rev(),
            )
            .rev()
            .find(|root| {
                // grab all the victim swaps and transactions and use the same
                // method to convert transfers into swaps thus align the searcher
                // swaps and victim swaps
                let actions = root.collect(
                    &TreeSearchBuilder::default()
                        .with_actions([Action::is_swap, Action::is_transfer]),
                );

                if actions.is_empty() {
                    return false
                }

                // collect actions and transform into raw swaps
                let (mut trigger_swaps, transfers): (Vec<_>, Vec<_>) = actions
                    .into_iter()
                    .split_actions((Action::try_swaps_merged, Action::try_transfer));

                let Ok(vic_info) = root.get_tx_info(arb_info.block_number, self.utils.db) else {
                    return false
                };
                let accounting_addr: FastHashSet<Address> =
                    vic_info.collect_address_set_for_accounting();

                let mut ignore_addresses = accounting_addr.clone();
                trigger_swaps.iter().for_each(|s| {
                    ignore_addresses.insert(s.pool);
                });
                trigger_swaps.extend(self.utils.try_create_swaps(&transfers, ignore_addresses));

                // look for  a intersection of trigger swaps and arb swaps where the pool is the
                // same and the assets are going in a different direction. if we find 1, we have
                // our trigger transaction

                // could hashmap and key but given that there are on average only 1 -3 victim
                // swaps iter is faster.
                trigger_swaps.into_iter().any(|trigger_swap| {
                    let NormalizedSwap { protocol, pool, token_in, token_out, .. } = trigger_swap;
                    swaps
                        .iter()
                        .find(|searcher_swap| {
                            searcher_swap.protocol == protocol
                            && searcher_swap.pool == pool
                            // only have to check 1
                            && searcher_swap.token_in == token_out
                            && searcher_swap.token_out == token_in
                        })
                        .map(|_| true)
                        .unwrap_or(false)
                })
            })
            .map(|root| root.tx_hash)
            .unwrap_or_default()
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
        let res = tx_info
            .is_searcher_of_type_with_count_threshold(MevType::AtomicArb, 20 * multiplier)
            || tx_info.is_labelled_searcher_of_type(MevType::AtomicArb)
            || tx_info.gas_details.coinbase_transfer.is_some() && tx_info.is_private;

        if !res {
            self.utils.get_metrics().inspect(|m| {
                m.branch_filtering_trigger(MevType::AtomicArb, "process_triangle_arb")
            });
        }
        res
    }

    fn is_cross_pair_or_stable_arb(&self, tx_info: &TxInfo, multiplier: u64) -> bool {
        let res = tx_info
            .is_searcher_of_type_with_count_threshold(MevType::AtomicArb, 20 * multiplier)
            || tx_info.is_labelled_searcher_of_type(MevType::AtomicArb)
            || tx_info.is_private
            || tx_info.gas_details.coinbase_transfer.is_some();
        if !res {
            self.utils.get_metrics().inspect(|m| {
                m.branch_filtering_trigger(MevType::AtomicArb, "is_cross_pair_or_stable_arb")
            });
        }
        res
    }

    fn is_long_tail(&self, tx_info: &TxInfo, multiplier: u64) -> bool {
        let res = tx_info
            .is_searcher_of_type_with_count_threshold(MevType::AtomicArb, 100 * multiplier)
            || tx_info.is_labelled_searcher_of_type(MevType::AtomicArb)
            || tx_info.is_private && tx_info.gas_details.coinbase_transfer.is_some()
            || tx_info.mev_contract.is_some();
        if !res {
            self.utils
                .get_metrics()
                .inspect(|m| m.branch_filtering_trigger(MevType::AtomicArb, "is_long_tail"));
        }
        res
    }

    fn is_stable_arb(&self, swaps: &[NormalizedSwap], jump_index: usize) -> bool {
        let token_bought = &swaps[jump_index - 1].token_out.symbol;
        let token_sold = &swaps[jump_index].token_in.symbol;

        let res = is_stable_pair(token_sold, token_bought);
        if !res {
            self.utils
                .get_metrics()
                .inspect(|m| m.branch_filtering_trigger(MevType::AtomicArb, "is_stable_arb"));
        }

        res
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

pub fn is_stable_pair(token_in: &str, token_out: &str) -> bool {
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
    use brontes_types::constants::USDT_ADDRESS;

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

    // TODO: This fails because we don't classify the DODO swap on this contract
    // https://etherscan.io/address/0x7ca7b5eaaf526d93705d28c1b47e9739595c90e7#code
    //
    #[brontes_macros::test]
    async fn test_misclassification() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.5).await;

        let tx = hex!("00044a090a5eb970334de119b680834ddcdd55cc34488c7446558e98d2660bfb").into();
        let config = InspectorTxRunConfig::new(Inspectors::AtomicArb)
            .with_mev_tx_hashes(vec![tx])
            .with_dex_prices()
            .with_expected_profit_usd(0.126)
            .with_gas_paid_usd(13.961);

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
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 2.5).await;

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
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 1.5).await;

        let config = InspectorTxRunConfig::new(Inspectors::AtomicArb)
            .with_mev_tx_hashes(vec![hex!(
                "3cfca5f7d00b7f204f6e1bd51e6113094c9fe8abebafd4354e423aca57d93a9b"
            )
            .into()])
            .with_dex_prices()
            .needs_tokens(vec![WETH_ADDRESS])
            .with_expected_profit_usd(4.08)
            .with_gas_paid_usd(154.68);

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

    #[brontes_macros::test]
    async fn test_more_seawise_weirdness() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.5).await;

        let config = InspectorTxRunConfig::new(Inspectors::AtomicArb)
            .with_mev_tx_hashes(vec![hex!(
                "739a2b975e3983e0f4c63a99ebd14a8dcd00d51c2eafc2a6ee13e507dcfa1523"
            )
            .into()])
            .with_dex_prices()
            .needs_tokens(vec![WETH_ADDRESS])
            .with_expected_profit_usd(28.06)
            .with_gas_paid_usd(75.75);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn assert_no_mev_0x() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.5).await;

        let config = InspectorTxRunConfig::new(Inspectors::AtomicArb)
            .with_mev_tx_hashes(vec![hex!(
                "bd3cccec96a23f62af9f99f185929022a048705b4e5f20c025bd5f023d10b7da"
            )
            .into()])
            .with_dex_prices()
            .needs_tokens(vec![WETH_ADDRESS]);

        inspector_util.assert_no_mev(config).await.unwrap();
    }

    #[brontes_macros::test]
    async fn assert_no_mev_1inch() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.5).await;

        let config = InspectorTxRunConfig::new(Inspectors::AtomicArb)
            .with_mev_tx_hashes(vec![hex!(
                "cb70044718a016a75c811209552b7af57f64b27e6a502221f96e991968accef4"
            )
            .into()])
            .with_dex_prices()
            .needs_tokens(vec![WETH_ADDRESS]);

        inspector_util.assert_no_mev(config).await.unwrap();
    }

    #[brontes_macros::test]
    async fn assert_no_simple_tri_swap() {
        let inspector_util = InspectorTestUtils::new(USDT_ADDRESS, 0.5).await;

        let config = InspectorTxRunConfig::new(Inspectors::AtomicArb)
            .with_mev_tx_hashes(vec![hex!(
                "ce1f462d3243bbeff016b4c6eabdfc7c6642b02b64de3de50a1a5d19cebcde1a"
            )
            .into()])
            .with_dex_prices()
            .needs_tokens(vec![WETH_ADDRESS]);

        inspector_util.assert_no_mev(config).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_eth_transfer_structure() {
        let inspector_util = InspectorTestUtils::new(USDT_ADDRESS, 0.5).await;
        let config = InspectorTxRunConfig::new(Inspectors::AtomicArb)
            .with_mev_tx_hashes(vec![hex!(
                "522824b872e68f3227350d65a9447d46d6cd039d70bd469f0de2477bc4333fbb"
            )
            .into()])
            .with_dex_prices()
            .needs_tokens(vec![WETH_ADDRESS]);

        inspector_util.assert_no_mev(config).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_very_big_atomic_arb() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.5).await;

        let config = InspectorTxRunConfig::new(Inspectors::AtomicArb)
            .with_mev_tx_hashes(vec![hex!(
                "358f46381b464f0195c0e39acdaa223fbf44a716e177b04febf01e3691247626"
            )
            .into()])
            .with_dex_prices()
            .needs_tokens(vec![WETH_ADDRESS])
            .with_expected_profit_usd(742_201.93)
            .with_gas_paid_usd(11.44);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_very_big_atomic_arb_2() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 5.5).await;

        let config = InspectorTxRunConfig::new(Inspectors::AtomicArb)
            .with_mev_tx_hashes(vec![hex!(
                "ed3248d5386237cfe12963e0d35e1541707cad4fdca43801f3799861e8adb9b5"
            )
            .into()])
            .with_dex_prices()
            .needs_tokens(vec![WETH_ADDRESS])
            .with_expected_profit_usd(70154.70)
            .with_gas_paid_usd(1458.25);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_not_zero_on_non_mev() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 5.5).await;

        let config = InspectorTxRunConfig::new(Inspectors::AtomicArb)
            .with_mev_tx_hashes(vec![hex!(
                "e4e6860fc2ae666c417a088caa96f62da073a8f4fb08ef74faf831407b84f0af"
            )
            .into()])
            .with_dex_prices()
            .needs_tokens(vec![WETH_ADDRESS])
            .with_expected_profit_usd(11.218)
            .with_gas_paid_usd(51.14);

        inspector_util.run_inspector(config, None).await.unwrap();
    }
}
