use std::sync::Arc;

use brontes_database::libmdbx::LibmdbxReader;
use brontes_metrics::inspectors::{OutlierMetrics, ProfitMetrics};
use brontes_types::{
    db::dex::PriceAt,
    mev::{Bundle, BundleData, Liquidation, MevType},
    normalized_actions::{accounting::ActionAccounting, Action},
    ActionIter, BlockData, FastHashSet, MultiBlockData, ToFloatNearest, TreeSearchBuilder, TxInfo,
};
use itertools::multizip;
use malachite::{num::basic::traits::Zero, Rational};
use reth_primitives::{b256, Address};
use itertools::Itertools;

use super::{MAX_PROFIT, MIN_PROFIT};
use crate::{shared_utils::SharedInspectorUtils, Inspector, Metadata};

pub struct LiquidationInspector<'db, DB: LibmdbxReader> {
    utils: SharedInspectorUtils<'db, DB>,
}

impl<'db, DB: LibmdbxReader> LiquidationInspector<'db, DB> {
    pub fn new(
        quote: Address,
        db: &'db DB,
        metrics: Option<OutlierMetrics>,
        profit_metrics: Option<ProfitMetrics>,
    ) -> Self {
        Self { utils: SharedInspectorUtils::new(quote, db, metrics, profit_metrics) }
    }
}

impl<DB: LibmdbxReader> Inspector for LiquidationInspector<'_, DB> {
    type Result = Vec<Bundle>;

    fn get_id(&self) -> &str {
        "Liquidation"
    }

    fn get_quote_token(&self) -> Address {
        self.utils.quote
    }

    fn inspect_block(&self, mut data: MultiBlockData) -> Self::Result {
        let block = data.per_block_data.pop().expect("no blocks");
        let BlockData { metadata, tree } = block;

        let ex = || {
            let (tx, liq): (Vec<_>, Vec<_>) = tree
                .clone()
                .collect_all(TreeSearchBuilder::default().with_actions([
                    Action::is_swap,
                    Action::is_liquidation,
                    Action::is_transfer,
                    Action::is_eth_transfer,
                    Action::is_aggregator,
                ]))
                .unzip();
            let tx_info = tree.get_tx_info_batch(&tx, self.utils.db);

            multizip((liq, tx_info))
                .filter_map(|(liq, info)| {
                    let info = info?;
                    let actions = self
                        .utils
                        .flatten_nested_actions_default(liq.into_iter())
                        .collect::<Vec<_>>();

                    self.calculate_liquidation(info, metadata.clone(), actions)
                })
                .collect::<Vec<_>>()
        };
        self.utils
            .get_metrics()
            .map(|m| m.run_inspector(MevType::Liquidation, ex))
            .unwrap_or_else(ex)
    }
}

impl<DB: LibmdbxReader> LiquidationInspector<'_, DB> {
    fn calculate_liquidation(
        &self,
        info: TxInfo,
        metadata: Arc<Metadata>,
        actions: Vec<Action>,
    ) -> Option<Bundle> {
        let (swaps, liqs): (Vec<_>, Vec<_>) = actions
            .iter()
            .cloned()
            .action_split((Action::try_swaps_merged, Action::try_liquidation));

        if liqs.is_empty() {
            tracing::debug!("no liquidation events");
            return None
        }

        let mev_addresses: FastHashSet<Address> = info.collect_address_set_for_accounting();
        let protocols = self.utils.get_related_protocols_liquidation(&actions);
        let protocols_str = protocols.iter().map(|p| p.to_string()).collect_vec();

        let deltas = actions
            .into_iter()
            .chain(info.get_total_eth_value().iter().cloned().map(Action::from))
            .filter(|a| a.is_eth_transfer() || a.is_transfer())
            .account_for_actions();

        let (rev, mut has_dex_price) = if let Some(rev) = self.utils.get_deltas_usd(
            info.tx_index,
            PriceAt::After,
            &mev_addresses,
            &deltas,
            metadata.clone(),
            false,
        ) {
            (Some(rev), true)
        } else {
            (Some(Rational::ZERO), false)
        };

        let gas_finalized =
            metadata.get_gas_price_usd(info.gas_details.gas_paid(), self.utils.quote);

        let mut profit_usd = rev
            .map(|rev| rev - &gas_finalized)
            .filter(|_| has_dex_price)
            .unwrap_or_default();

        if profit_usd >= MAX_PROFIT || profit_usd <= MIN_PROFIT {
            has_dex_price = false;
            profit_usd = Rational::ZERO;
        }

        let header = self.utils.build_bundle_header(
            vec![deltas],
            vec![info.tx_hash],
            &info,
            profit_usd.clone().to_float(),
            &[info.gas_details],
            metadata.clone(),
            MevType::Liquidation,
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

        let new_liquidation = Liquidation {
            block_number:        metadata.block_num,
            liquidation_tx_hash: info.tx_hash,
            trigger:             b256!(),
            liquidation_swaps:   swaps,
            liquidations:        liqs,
            gas_details:         info.gas_details,
            profit_usd:          profit_usd.clone().to_float(),
            protocols:           protocols_str,
        };

        self.utils.get_profit_metrics().inspect(|m| {
            m.publish_profit_metrics(MevType::Liquidation, protocols, profit_usd.to_float(), info.timeboosted)
        });
        Some(Bundle { header, data: BundleData::Liquidation(new_liquidation) })
    }
}

#[cfg(test)]
mod tests {

    use alloy_primitives::hex;

    use crate::{
        test_utils::{InspectorTestUtils, InspectorTxRunConfig, USDC_ADDRESS},
        Inspectors,
    };

    #[brontes_macros::test]
    async fn test_aave_v3_liquidation() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 6.0).await;

        let config = InspectorTxRunConfig::new(Inspectors::Liquidations)
            .with_mev_tx_hashes(vec![hex!(
                "dd951e0fc5dc4c98b8daaccdb750ff3dc9ad24a7f689aad2a088757266ab1d55"
            )
            .into()])
            .needs_tokens(vec![
                hex!("2260fac5e5542a773aa44fbcfedf7c193bc2c599").into(),
                hex!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").into(),
            ])
            .with_dex_prices()
            .with_gas_paid_usd(2793.9)
            .with_expected_profit_usd(71.593);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_aave_v2_liquidation() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 5.0).await;

        let config = InspectorTxRunConfig::new(Inspectors::Liquidations)
            .with_mev_tx_hashes(vec![hex!(
                "725551f77f94f0ff01046aa4f4b93669d689f7eda6bb8cd87e2be780935eb2db"
            )
            .into()])
            .needs_token(hex!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").into())
            .with_dex_prices()
            .with_gas_paid_usd(638.71) //TODO: Joe I am changing this for now because your quotes data seems to still be
            // incorrect. Please fix it, the previous value was 636.54
            .with_expected_profit_usd(128.11); // Same here previous value was: 129.23

        inspector_util.run_inspector(config, None).await.unwrap();
    }
}
