use std::{collections::HashSet, sync::Arc};

use brontes_database::{Metadata, Pair};
use brontes_database_libmdbx::Libmdbx;
use brontes_types::{
    classified_mev::{ClassifiedMev, Liquidation, MevType, SpecificMev},
    normalized_actions::{Actions, NormalizedLiquidation, NormalizedSwap},
    tree::{BlockTree, GasDetails, Node, Root},
    ToFloatNearest,
};
use malachite::{num::basic::traits::Zero, Rational};
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use reth_primitives::{b256, Address, B256};

use crate::{shared_utils::SharedInspectorUtils, Inspector};

pub struct LiquidationInspector<'db> {
    inner: SharedInspectorUtils<'db>,
}

impl<'db> LiquidationInspector<'db> {
    pub fn new(quote: Address, db: &'db Libmdbx) -> Self {
        Self { inner: SharedInspectorUtils::new(quote, db) }
    }
}

#[async_trait::async_trait]
impl Inspector for LiquidationInspector<'_> {
    async fn process_tree(
        &self,
        tree: Arc<BlockTree<Actions>>,
        metadata: Arc<Metadata>,
    ) -> Vec<(ClassifiedMev, Box<dyn SpecificMev>)> {
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

impl LiquidationInspector<'_> {
    fn calculate_liquidation(
        &self,
        tx_hash: B256,
        idx: usize,
        mev_contract: Address,
        eoa: Address,
        metadata: Arc<Metadata>,
        actions: Vec<Actions>,
        gas_details: &GasDetails,
    ) -> Option<(ClassifiedMev, Box<dyn SpecificMev>)> {
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
            .usd_delta_by_address(idx, deltas, metadata.clone(), false)?;
        let mev_profit_collector = self.inner.profit_collectors(&swap_profit);

        let liq_profit = liqs
            .par_iter()
            .filter_map(|liq| {
                let repaid_debt_usd = self.inner.calculate_dex_usd_amount(
                    idx,
                    liq.debt_asset,
                    liq.covered_debt,
                    &metadata,
                )?;
                let collected_collateral = self.inner.calculate_dex_usd_amount(
                    idx,
                    liq.collateral_asset,
                    liq.liquidated_collateral,
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

        let mev = ClassifiedMev {
            mev_tx_index: idx as u64,
            block_number: metadata.block_num,
            eoa,
            tx_hash,
            mev_contract,
            mev_profit_collector,
            finalized_profit_usd: profit_usd.to_float(),
            finalized_bribe_usd: gas_finalized.to_float(),
            mev_type: MevType::Liquidation,
        };

        let new_liquidation = Liquidation {
            liquidation_tx_hash: tx_hash,
            trigger:             b256!(),
            liquidation_swaps:   swaps,
            liquidations:        liqs,
            gas_details:         gas_details.clone(),
        };

        Some((mev, Box::new(new_liquidation)))
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, str::FromStr, time::SystemTime};

    use alloy_primitives::hex;
    use brontes_classifier::Classifier;
    use reth_primitives::U256;
    use serial_test::serial;

    use super::*;
    use crate::test_utils::{InspectorTestUtils, InspectorTxRunConfig, USDC_ADDRESS};

    #[tokio::test]
    #[serial]
    async fn test_aave_v3_liquidation() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 1.0);

        let config = InspectorTxRunConfig::new(MevType::Liquidation)
            .with_block(19042179)
            .with_dex_prices()
            .with_expected_gas_used(2792.487)
            .with_expected_profit_usd(76.75);

        inspector_util
            .run_inspector::<Liquidation>(config, None)
            .await
            .unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_aave_v2_liquidation() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 1.0);

        let config = InspectorTxRunConfig::new(MevType::Liquidation)
            .with_block(18979710)
            .with_dex_prices()
            .with_expected_gas_used(636.54)
            .with_expected_profit_usd(129.23);

        inspector_util
            .run_inspector::<Liquidation>(config, None)
            .await
            .unwrap();
    }
}
