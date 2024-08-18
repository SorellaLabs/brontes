use std::sync::Arc;

use alloy_primitives::Address;
use brontes_types::{
    db::{metadata::Metadata, token_info::TokenInfoWithAddress, traits::LibmdbxReader},
    display::utils::format_etherscan_url,
    mev::{Bundle, BundleData, MevType},
    normalized_actions::{accounting::ActionAccounting, Action, NormalizedSwap},
    tree::BlockTree,
    BlockData, FastHashMap, MultiBlockData,
};
use itertools::multizip;
use malachite::{num::basic::traits::Zero, Rational};
use tracing::trace;

use super::JitInspector;
use crate::{
    cex_dex::markout::{CexDexMarkoutInspector, CexDexProcessing},
    Inspector,
};

/// Jit cex dex occurs in two cases:
///     1) a cex dex arb on a pool
///     2) a user swap on the pool where the volume is greater than the amount
///        the market marker would fill to move the pool to the true price.
///
/// When this occurs market makers add liquidity to
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

    fn inspect_block(&self, mut data: MultiBlockData) -> Self::Result {
        let block = data.per_block_data.pop().expect("no blocks");
        let BlockData { metadata, tree } = block;
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
        if metadata.cex_trades.is_none() {
            tracing::warn!("no cex trades for block");
            return vec![]
        }
        // call inner to avoid metrics
        let jit_bundles = self.jit.inspect_block_inner(tree.clone(), metadata.clone());
        jit_bundles
            .into_iter()
            .filter_map(|jits| {
                tracing::trace!(
                    "Checking if classified JITs are actually JIT Cex Dex- {:#?}",
                    jits
                );
                let BundleData::Jit(jit) = jits.data else { return None };
                let details = [jit.backrun_burn_gas_details, jit.frontrun_mint_gas_details];
                let tx_info = tree.get_tx_info(jits.header.tx_hash, self.jit.utils.db)?;

                if !tx_info.is_searcher_of_type_with_count_threshold(MevType::JitCexDex, 10) {
                    return None
                }

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
                    dex_swaps.clone(),
                    &metadata,
                    tx_info.is_searcher_of_type(MevType::JitCexDex)
                        || tx_info.is_labelled_searcher_of_type(MevType::JitCexDex),
                    &tx_info,
                )?;

                self.cex_dex.gas_accounting(
                    &mut possible_cex_dex,
                    &tx_info.gas_details,
                    metadata.clone(),
                );

                let (profit_usd, cex_dex, trade_prices) = self.cex_dex.filter_possible_cex_dex(
                    possible_cex_dex,
                    &tx_info,
                    metadata.clone(),
                )?;

                let price_map =
                    trade_prices
                        .into_iter()
                        .fold(FastHashMap::default(), |mut acc, x| {
                            acc.insert(x.token0, x.price0);
                            acc.insert(x.token1, x.price1);
                            acc
                        });

                let deltas = dex_swaps
                    .into_iter()
                    .map(Action::from)
                    .account_for_actions();

                let header = self.jit.utils.build_bundle_header(
                    vec![deltas],
                    vec![tx_info.tx_hash],
                    &tx_info,
                    profit_usd,
                    &details,
                    metadata.clone(),
                    MevType::JitCexDex,
                    false,
                    |_, token, amount| Some(price_map.get(&token)? * amount),
                );

                Some(Bundle { header, data: cex_dex })
            })
            .collect::<Vec<_>>()
    }
}
