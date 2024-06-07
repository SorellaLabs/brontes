use std::sync::Arc;

use alloy_primitives::Address;
use brontes_core::LibmdbxReader;
use brontes_types::{
    db::{metadata::Metadata, token_info::TokenInfoWithAddress},
    display::utils::format_etherscan_url,
    mev::{Bundle, BundleData, MevType},
    normalized_actions::{Action, NormalizedSwap},
    tree::BlockTree,
    FastHashMap,
};
use itertools::multizip;
use malachite::{num::basic::traits::Zero, Rational};
use tracing::trace;

use crate::{
    cex_dex_markout::{CexDexMarkoutInspector, CexDexProcessing},
    jit::JitInspector,
    Inspector,
};

/// jit cex dex happens when two things are present.
/// 1) a cex dex arb on a pool
/// 2) a user swap on the pool where the volume
/// is greater than the amount the market marker would
/// fill to move the pool to the true price.
///
/// when this occurs market makers add liquidity to
/// the pool at a price that is worse than true price and get filled
/// more volume than they would otherwise from the user swapping through.
pub struct JitCexDex<'db, DB: LibmdbxReader> {
    pub cex_dex: CexDexMarkoutInspector<'db, DB>,
    pub jit:     JitInspector<'db, DB>,
}

impl<DB: LibmdbxReader> Inspector for JitCexDex<'_, DB> {
    type Result = Vec<Bundle>;

    fn get_id(&self) -> &str {
        "JitCexDex"
    }

    fn get_quote_token(&self) -> Address {
        self.jit.utils.quote
    }

    fn inspect_block(&self, tree: Arc<BlockTree<Action>>, metadata: Arc<Metadata>) -> Self::Result {
        self.jit
            .utils
            .get_metrics()
            .map(|m| {
                m.run_inspector(MevType::JitCexDex, || {
                    self.inspect_block_inner(tree.clone(), metadata.clone())
                })
            })
            .unwrap_or_else(|| self.inspect_block_inner(tree, metadata))
    }
}

impl<DB: LibmdbxReader> JitCexDex<'_, DB> {
    fn inspect_block_inner(
        &self,
        tree: Arc<BlockTree<Action>>,
        metadata: Arc<Metadata>,
    ) -> Vec<Bundle> {
        // call inner to avoid metrics
        let jit_bundles = self.jit.inspect_block_inner(tree.clone(), metadata.clone());
        jit_bundles
            .into_iter()
            .filter_map(|jits| {
                tracing::trace!("trying jit to see if cexdex -{:#?}", jits);
                let BundleData::Jit(jit) = jits.data else { return None };
                let tx_info = tree.get_tx_info(jits.header.tx_hash, self.jit.utils.db)?;
                let mut mint_burn_deltas: FastHashMap<
                    Address,
                    FastHashMap<TokenInfoWithAddress, Rational>,
                > = FastHashMap::default();

                jit.frontrun_mints.into_iter().for_each(|mint| {
                    for (token, amount) in multizip((mint.token, mint.amount)) {
                        *mint_burn_deltas
                            .entry(mint.pool)
                            .or_default()
                            .entry(token)
                            .or_default() -= amount;
                    }
                });

                jit.backrun_burns.into_iter().for_each(|burn| {
                    for (token, amount) in multizip((burn.token, burn.amount)) {
                        *mint_burn_deltas
                            .entry(burn.pool)
                            .or_default()
                            .entry(token)
                            .or_default() += amount;
                    }
                });

                let dex_swaps = mint_burn_deltas
                    .into_iter()
                    .map(|(pool, tokens)| {
                        // for each pool, there is some token delta that occurs, this will be amount
                        // in amount out based on which is negative and
                        // which is positive
                        let mut amount_out = Default::default();
                        let mut amount_in = Default::default();
                        let mut token_in = Default::default();
                        let mut token_out = Default::default();

                        for (token, delta) in tokens.into_iter().take(2) {
                            if delta > Rational::ZERO {
                                amount_out = delta;
                                token_out = token;
                            } else {
                                amount_in = delta;
                                token_in = token;
                            }
                        }
                        // make sure positive val
                        amount_in = -amount_in;

                        NormalizedSwap {
                            pool,
                            amount_out,
                            amount_in,
                            token_in,
                            token_out,
                            from: jits.header.mev_contract.unwrap_or(jits.header.eoa),
                            recipient: jits.header.mev_contract.unwrap_or(jits.header.eoa),
                            ..Default::default()
                        }
                    })
                    .collect::<Vec<_>>();

                if self.cex_dex.is_triangular_arb(&dex_swaps) {
                    trace!(
                        target: "brontes::cex-dex-markout",
                        "Filtered out CexDex because it is a triangular arb\n Tx: {}",
                        format_etherscan_url(&tx_info.tx_hash)
                    );
                    self.cex_dex.utils.get_metrics().inspect(|m| {
                        m.branch_filtering_trigger(MevType::JitCexDex, "is_triangular_arb")
                    });

                    return None
                }

                let mut possible_cex_dex: CexDexProcessing = self.cex_dex.detect_cex_dex(
                    dex_swaps,
                    &metadata,
                    tx_info.is_searcher_of_type(MevType::JitCexDex)
                        || tx_info.is_labelled_searcher_of_type(MevType::JitCexDex),
                    tx_info.tx_hash,
                )?;

                self.cex_dex.gas_accounting(
                    &mut possible_cex_dex,
                    &tx_info.gas_details,
                    metadata.clone(),
                );

                let (profit_usd, cex_dex) = self.cex_dex.filter_possible_cex_dex(
                    possible_cex_dex,
                    &tx_info,
                    metadata.clone(),
                )?;

                let header = self.jit.utils.build_bundle_header_jit_cex_dex(
                    jits.header,
                    &tx_info,
                    profit_usd,
                    &[tx_info.gas_details],
                    metadata.clone(),
                    MevType::JitCexDex,
                    false,
                );

                Some(Bundle { header, data: cex_dex })
            })
            .collect::<Vec<_>>()
    }
}

#[cfg(test)]
mod tests {

    use brontes_types::constants::USDT_ADDRESS;

    use crate::{
        test_utils::{InspectorTestUtils, InspectorTxRunConfig},
        Inspectors,
    };

    #[brontes_macros::test]
    async fn test_jit_cex_dex() {
        let inspector_util = InspectorTestUtils::new(USDT_ADDRESS, 0.5).await;

        let config = InspectorTxRunConfig::new(Inspectors::JitCexDex)
            .with_block(18305720)
            .with_gas_paid_usd(38.31)
            .with_expected_profit_usd(134.70);

        inspector_util.run_inspector(config, None).await.unwrap();
    }
}
