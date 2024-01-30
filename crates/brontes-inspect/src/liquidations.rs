use std::{collections::HashSet, sync::Arc};

use brontes_database::libmdbx::{Libmdbx, LibmdbxReader};
use brontes_types::{
    mev::{Bundle, BundleData, BundleHeader, Liquidation, MevType, TokenProfit, TokenProfits},
    normalized_actions::{Actions, NormalizedLiquidation, NormalizedSwap},
    pair::Pair,
    tree::{BlockTree, GasDetails, Node, Root},
    ToFloatNearest,
};
use hyper::header;
use malachite::{num::basic::traits::Zero, Rational};
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use reth_primitives::{b256, Address, B256};

use crate::{shared_utils::SharedInspectorUtils, Inspector, MetadataCombined};

pub struct LiquidationInspector<'db, DB: LibmdbxReader> {
    inner: SharedInspectorUtils<'db, DB>,
}

impl<'db, DB: LibmdbxReader> LiquidationInspector<'db, DB> {
    pub fn new(quote: Address, db: &'db DB) -> Self {
        Self { inner: SharedInspectorUtils::new(quote, db) }
    }
}

#[async_trait::async_trait]
impl<DB: LibmdbxReader> Inspector for LiquidationInspector<'_, DB> {
    async fn process_tree(
        &self,
        tree: Arc<BlockTree<Actions>>,
        metadata: Arc<MetadataCombined>,
    ) -> Vec<Bundle> {
        let liq_txs = tree.collect_all(|node| {
            (
                node.data.is_liquidation() || node.data.is_swap(),
                node.subactions
                    .iter()
                    .any(|action| action.is_liquidation() || action.is_swap()),
            )
        });

        liq_txs
            .into_par_iter()
            .filter_map(|(tx_hash, liq)| {
                let root = tree.get_root(tx_hash)?;
                if root.head.data.is_revert() {
                    return None
                }
                let eoa = root.head.address;
                let mev_contract = root.head.data.get_to_address();
                let idx = root.get_block_position();
                let gas_details = tree.get_gas_details(tx_hash)?;

                self.calculate_liquidation(
                    tx_hash,
                    idx,
                    mev_contract,
                    eoa,
                    metadata.clone(),
                    liq,
                    gas_details,
                )
            })
            .collect::<Vec<_>>()
    }
}

impl<DB: LibmdbxReader> LiquidationInspector<'_, DB> {
    fn calculate_liquidation(
        &self,
        tx_hash: B256,
        idx: usize,
        mev_contract: Address,
        eoa: Address,
        metadata: Arc<MetadataCombined>,
        actions: Vec<Actions>,
        gas_details: &GasDetails,
    ) -> Option<Bundle> {
        let swaps = actions
            .iter()
            .filter_map(|action| if let Actions::Swap(swap) = action { Some(swap) } else { None })
            .cloned()
            .collect::<Vec<_>>();

        let liqs = actions
            .iter()
            .filter_map(
                |action| {
                    if let Actions::Liquidation(liq) = action {
                        Some(liq)
                    } else {
                        None
                    }
                },
            )
            .cloned()
            .collect::<Vec<_>>();

        if liqs.is_empty() {
            return None
        }

        let deltas = self.inner.calculate_token_deltas(&vec![actions]);
        let swap_profit = self
            .inner
            .usd_delta_by_address(idx, &deltas, metadata.clone(), false)?;
        let mev_profit_collector = self.inner.profit_collectors(&swap_profit);

        let token_profits = TokenProfits {
            profits: mev_profit_collector
                .iter()
                .filter_map(|address| deltas.get(address).map(|d| (address, d)))
                .flat_map(|(address, delta)| {
                    delta.iter().map(|(token, amount)| {
                        let usd_value = metadata
                            .dex_quotes
                            .price_at_or_before(Pair(*token, self.inner.quote), idx)
                            .unwrap_or(Rational::ZERO)
                            .to_float()
                            * amount.clone().to_float();
                        TokenProfit {
                            profit_collector: *address,
                            token: *token,
                            amount: amount.clone().to_float(),
                            usd_value,
                        }
                    })
                })
                .collect(),
        };

        let liq_profit = liqs
            .par_iter()
            .filter_map(|liq| {
                let repaid_debt_usd = self.inner.calculate_dex_usd_amount(
                    idx,
                    liq.debt_asset.address,
                    &liq.covered_debt,
                    &metadata,
                )?;
                let collected_collateral = self.inner.calculate_dex_usd_amount(
                    idx,
                    liq.collateral_asset.address,
                    &liq.liquidated_collateral,
                    &metadata,
                )?;
                Some(collected_collateral - repaid_debt_usd)
            })
            .sum::<Rational>();

        let rev_usd = swap_profit
            .values()
            .fold(Rational::ZERO, |acc, delta| acc + delta)
            + liq_profit;

        let gas_finalized = metadata.get_gas_price_usd(gas_details.gas_paid());

        let profit_usd = rev_usd - &gas_finalized;

        let header = BundleHeader {
            tx_index: idx as u64,
            block_number: metadata.block_num,
            eoa,
            tx_hash,
            mev_contract,
            mev_profit_collector,
            profit_usd: profit_usd.to_float(),
            token_profits,
            bribe_usd: gas_finalized.to_float(),
            mev_type: MevType::Liquidation,
        };

        let new_liquidation = Liquidation {
            liquidation_tx_hash: tx_hash,
            trigger:             b256!(),
            liquidation_swaps:   swaps,
            liquidations:        liqs,
            gas_details:         gas_details.clone(),
        };

        Some(Bundle { header, data: BundleData::Liquidation(new_liquidation) })
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, str::FromStr, time::SystemTime};

    use alloy_primitives::{hex, U256};
    use brontes_classifier::Classifier;
    use serial_test::serial;

    use super::*;
    use crate::{
        test_utils::{InspectorTestUtils, InspectorTxRunConfig, USDC_ADDRESS},
        Inspectors,
    };

    #[tokio::test]
    #[serial]
    async fn test_aave_v3_liquidation() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 1.0);

        let config = InspectorTxRunConfig::new(Inspectors::Liquidations)
            .with_block(19042179)
            .with_dex_prices()
            .with_gas_paid_usd(2792.487)
            .with_expected_profit_usd(71.593);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_aave_v2_liquidation() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 1.0);

        let config = InspectorTxRunConfig::new(Inspectors::Liquidations)
            .with_block(18979710)
            .with_dex_prices()
            .with_gas_paid_usd(636.54)
            .with_expected_profit_usd(129.23);

        inspector_util.run_inspector(config, None).await.unwrap();
    }
}
