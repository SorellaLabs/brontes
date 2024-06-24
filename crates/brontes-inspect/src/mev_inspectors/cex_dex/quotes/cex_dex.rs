//! This module implements the `CexDexQuotesInspector`, a specialized inspector
//! designed to detect arbitrage opportunities between centralized
//! exchanges (CEXs) and decentralized exchanges (DEXs).
//!
//! ## Overview
//!
//! A Cex-Dex arbitrage occurs when a trader exploits the price difference
//! between a CEX and a DEX. The trader buys an undervalued asset on the DEX and
//! sells it on the CEX.
//!
//!
//! ## Methodology
//!
//! The `CexDexQuotesInspector` systematically identifies arbitrage
//! opportunities between CEXs and DEXs by analyzing transactions containing
//! swap actions.
//!
//! ### Step 1: Collect Transactions
//! All transactions containing swap actions are collected from the block tree
//! using `collect_all`.
//!
//! ### Step 2: Detect Arbitrage Opportunities
//! For each transaction with swaps, the inspector:
//!   - Retrieves CEX quotes for the swapped tokens for each exchange with
//!     `cex_quotes_for_swap`.
//!   - Calculates PnL post Cex & Dex fee and identifies arbitrage legs with
//!     `detect_cex_dex_opportunity`, considering both direct and intermediary
//!     token quotes.
//!   - Assembles `PossibleCexDexLeg` instances, for each swap, containing the
//!     swap action and the potential arbitrage legs i.e the different
//!     arbitrages that can be done for each exchange.
//!
//! ### Step 3: Profit Calculation and Gas Accounting
//! The inspector filters for the most profitable arbitrage path per swap i.e
//! for a given swap it gets the exchange with the highest profit
//! through `filter_most_profitable_leg`. It then gets the total potential
//! profit, and accounts for gas costs with `gas_accounting` to calculate the
//! transactions final PnL.
//!
//! ### Step 4: Validation and Bundle Construction
//! Arbitrage opportunities are validated and false positives minimized in
//! `filter_possible_cex_dex`. Valid opportunities are bundled into
//! `BundleData::CexDex` instances.
use std::{
    cmp::{max, min},
    sync::Arc,
};

use brontes_database::libmdbx::LibmdbxReader;
use brontes_metrics::inspectors::OutlierMetrics;
use brontes_types::{
    db::{
        cex::{CexExchange, FeeAdjustedQuote},
        dex::PriceAt,
    },
    mev::{ArbDetails, ArbPnl, Bundle, BundleData, MevType},
    normalized_actions::{accounting::ActionAccounting, Action, NormalizedSwap},
    pair::Pair,
    tree::{BlockTree, GasDetails},
    ActionIter, ToFloatNearest, TreeSearchBuilder, TxInfo,
};
use malachite::{num::basic::traits::Two, Rational};
use reth_primitives::{Address, TxHash};
use tracing::debug;

use super::types::{log_price_delta, CexDexProcessing, ExchangeLeg, PossibleCexDex};

pub const FILTER_THRESHOLD: u64 = 20;
pub const HIGH_PROFIT_THRESHOLD: Rational = Rational::const_from_unsigned(10000);
use itertools::Itertools;

use crate::{shared_utils::SharedInspectorUtils, Inspector, Metadata};
pub struct CexDexQuotesInspector<'db, DB: LibmdbxReader> {
    utils:         SharedInspectorUtils<'db, DB>,
    cex_exchanges: Vec<CexExchange>,
}

impl<'db, DB: LibmdbxReader> CexDexQuotesInspector<'db, DB> {
    /// Constructs a new `CexDexQuotesInspector`.
    ///
    /// # Arguments
    ///
    /// * `quote` - The address of the quote asset
    /// * `db` - Database reader to our local libmdbx database
    /// * `cex_exchanges` - List of centralized exchanges to consider for
    ///   arbitrage.
    pub fn new(
        quote: Address,
        db: &'db DB,
        cex_exchanges: &[CexExchange],
        metrics: Option<OutlierMetrics>,
    ) -> Self {
        Self {
            utils:         SharedInspectorUtils::new(quote, db, metrics),
            cex_exchanges: cex_exchanges.to_owned(),
        }
    }
}

impl<DB: LibmdbxReader> Inspector for CexDexQuotesInspector<'_, DB> {
    type Result = Vec<Bundle>;

    fn get_id(&self) -> &str {
        "CexDex"
    }

    fn get_quote_token(&self) -> Address {
        self.utils.quote
    }

    /// Processes the block tree to find CEX-DEX arbitrage
    /// opportunities. This is the entry point for the inspection process,
    /// identifying transactions that include swap actions.
    ///
    /// # Arguments
    /// * `tree` - A shared reference to the block tree.
    /// * `metadata` - Shared metadata struct containing:
    ///     - `cex_quotes` - CEX quotes
    ///     - `dex_quotes` - DEX quotes
    ///     - `private_flow` - Set of private transactions that were not seen in
    ///       the mempool
    ///     - `relay & p2p_timestamp` - When the block was first sent to a relay
    ///       & when it was first seen in the p2p network
    ///
    ///
    /// # Returns
    /// A vector of `Bundle` instances representing classified CEX-DEX arbitrage
    fn inspect_block(&self, tree: Arc<BlockTree<Action>>, metadata: Arc<Metadata>) -> Self::Result {
        tree.clone()
            .collect_all(TreeSearchBuilder::default().with_actions([
                Action::is_swap,
                Action::is_transfer,
                Action::is_eth_transfer,
                Action::is_aggregator,
            ]))
            .filter_map(|(tx, swaps)| {
                let tx_info = tree.get_tx_info(tx, self.utils.db)?;

                // Return early if the tx is a solver settling trades
                if let Some(contract_type) = tx_info.contract_type.as_ref() {
                    if contract_type.is_solver_settlement() || contract_type.is_defi_automation() {
                        return None
                    }
                }

                let deltas = swaps.clone().into_iter().account_for_actions();
                let dex_swaps = self
                    .utils
                    .flatten_nested_actions(swaps.into_iter(), &|action| action.is_swap())
                    .collect_action_vec(Action::try_swaps_merged);

                // Early return to filter out triangular arbitrage
                if self.is_triangular_arb(&dex_swaps) {
                    return None
                }

                let mut possible_cex_dex: CexDexProcessing =
                    self.detect_cex_dex(dex_swaps, &metadata, &tx_info.tx_hash)?;

                self.gas_accounting(&mut possible_cex_dex, &tx_info.gas_details, metadata.clone());

                let (profit_usd, cex_dex) =
                    self.filter_possible_cex_dex(possible_cex_dex, &tx_info)?;

                let header = self.utils.build_bundle_header(
                    vec![deltas],
                    vec![tx_info.tx_hash],
                    &tx_info,
                    profit_usd,
                    PriceAt::After,
                    &[tx_info.gas_details],
                    metadata.clone(),
                    MevType::CexDex,
                    false,
                );

                Some(Bundle { header, data: cex_dex })
            })
            .collect::<Vec<_>>()
    }
}

impl<DB: LibmdbxReader> CexDexQuotesInspector<'_, DB> {
    pub fn detect_cex_dex(
        &self,
        dex_swaps: Vec<NormalizedSwap>,
        metadata: &Metadata,
        tx_hash: &TxHash,
    ) -> Option<CexDexProcessing> {
        let quotes = self
            .cex_exchanges
            .iter()
            .map(|exchange| self.cex_quotes_for_swap(&dex_swaps, metadata, exchange, tx_hash))
            .collect_vec();

        let mut transposed_quotes: Vec<Vec<&Option<FeeAdjustedQuote>>> =
            vec![Vec::new(); quotes[0].len()];

        quotes.iter().for_each(|q| {
            q.iter().enumerate().for_each(|(index, quote)| {
                transposed_quotes[index].push(quote);
            })
        });

        let mut quotes_vwam = Vec::new();

        for (index, quotes) in transposed_quotes.iter().enumerate() {
            let some_quotes: Vec<&FeeAdjustedQuote> =
                quotes.iter().filter_map(|quote| quote.as_ref()).collect();

            if some_quotes.is_empty() {
                return None
            } else {
                quotes_vwam.push(
                    metadata
                        .cex_quotes
                        .get_volume_weighted_quote(&some_quotes, &dex_swaps[index]),
                );
            }
        }

        let global_vwam_cex_dex =
            self.detect_cex_dex_opportunity(&dex_swaps, quotes_vwam, metadata, tx_hash);

        let per_exchange_pnl = quotes
            .into_iter()
            .map(|quotes| self.detect_cex_dex_opportunity(&dex_swaps, quotes, metadata, tx_hash))
            .collect();

        let mut cex_dex = CexDexProcessing {
            dex_swaps,
            global_vmam_cex_dex: global_vwam_cex_dex,
            per_exchange_pnl,
            max_profit: None,
        };

        cex_dex.construct_max_profit_route()?;

        Some(cex_dex)
    }

    /// Detects potential CEX-DEX arbitrage opportunities for a sequence of
    /// swaps
    ///
    /// # Arguments
    ///
    /// * `dex_swaps` - The DEX swaps to analyze.
    /// * `metadata` - Combined metadata for additional context in analysis.
    /// * `cex_prices` - Fee adjusted CEX quotes for the corresponding swaps.
    ///
    /// # Returns
    ///
    /// An option containing a `PossibleCexDex` if an opportunity is found,
    /// otherwise `None`.
    pub fn detect_cex_dex_opportunity(
        &self,
        dex_swaps: &[NormalizedSwap],
        cex_prices: Vec<Option<FeeAdjustedQuote>>,
        metadata: &Metadata,
        tx_hash: &TxHash,
    ) -> Option<PossibleCexDex> {
        PossibleCexDex::from_exchange_legs(
            dex_swaps
                .iter()
                .zip(cex_prices)
                .map(|(dex_swap, quote)| {
                    if let Some(q) = quote {
                        self.profit_classifier(dex_swap, q, metadata, tx_hash)
                    } else {
                        None
                    }
                })
                .collect_vec(),
        )
    }

    /// For a given DEX swap & CEX quote, calculates the potential profit from
    /// buying on DEX and selling on CEX.
    fn profit_classifier(
        &self,
        dex_swap: &NormalizedSwap,
        cex_quote: FeeAdjustedQuote,
        metadata: &Metadata,
        tx_hash: &TxHash,
    ) -> Option<ExchangeLeg> {
        // If the price difference between the DEX and CEX is greater than 2x, the
        // quote is likely invalid.
        let dex_swap_rate = dex_swap.swap_rate();
        let smaller = min(&dex_swap_rate, &cex_quote.price_maker.1);
        let larger = max(&dex_swap_rate, &cex_quote.price_maker.1);

        if smaller * Rational::TWO < *larger {
            log_price_delta(
                dex_swap.token_in_symbol(),
                dex_swap.token_out_symbol(),
                &cex_quote.exchange,
                dex_swap.swap_rate().clone().to_float(),
                cex_quote.price_maker.1.clone().to_float(),
                &dex_swap.token_in.address,
                &dex_swap.token_out.address,
            );

            return None
        }

        // A positive delta indicates potential profit from buying on DEX
        // and selling on CEX.
        let maker_taker_mid = cex_quote.maker_taker_mid();

        let maker_mid_delta = &maker_taker_mid.0 - &dex_swap_rate;
        let taker_mid_delta = &maker_taker_mid.1 - &dex_swap_rate;

        let token_price = metadata.cex_quotes.get_quote_direct_or_via_intermediary(
            &Pair(dex_swap.token_in.address, self.utils.quote),
            &cex_quote.exchange,
            None,
            Some(tx_hash),
        )?;

        let token_maker_taker_mid = token_price.maker_taker_mid();

        let pnl_mid = (
            &maker_mid_delta * &dex_swap.amount_out * &token_maker_taker_mid.0,
            &taker_mid_delta * &dex_swap.amount_out * &token_maker_taker_mid.1,
        );

        let maker_ask_delta = &cex_quote.price_maker.1 - &dex_swap_rate;
        let taker_ask_delta = &cex_quote.price_taker.1 - &dex_swap_rate;

        let token_maker_taker_ask = token_price.maker_taker_ask();

        let pnl_ask = (
            &maker_ask_delta * &dex_swap.amount_out * &token_maker_taker_ask.0,
            &taker_ask_delta * &dex_swap.amount_out * &token_maker_taker_ask.1,
        );

        Some(ExchangeLeg {
            cex_quote: cex_quote.clone(),
            pnl:       ArbPnl { maker_taker_mid: pnl_mid, maker_taker_ask: pnl_ask },
        })
    }

    /// Retrieves CEX quotes for a DEX swap, analyzing both direct and
    /// intermediary token pathways.
    fn cex_quotes_for_swap(
        &self,
        dex_swaps: &[NormalizedSwap],
        metadata: &Metadata,
        exchange: &CexExchange,
        tx_hash: &TxHash,
    ) -> Vec<Option<FeeAdjustedQuote>> {
        dex_swaps
            .iter()
            .map(|dex_swap| {
                let pair = Pair(dex_swap.token_out.address, dex_swap.token_in.address);

                metadata
                    .cex_quotes
                    .get_quote_direct_or_via_intermediary(
                        &pair,
                        exchange,
                        Some(dex_swap),
                        Some(tx_hash),
                    )
                    .or_else(|| {
                        debug!(
                            "No CEX quote found for pair: {}, {} at exchange: {:?}",
                            dex_swap.token_in_symbol(),
                            dex_swap.token_out_symbol(),
                            exchange
                        );
                        None
                    })
            })
            .collect()
    }

    /// Accounts for gas costs in the calculation of potential arbitrage
    /// profits. This function calculates the final pnl for the transaction by
    /// subtracting gas costs from the total potential arbitrage profits.
    fn gas_accounting(
        &self,
        cex_dex: &mut CexDexProcessing,
        gas_details: &GasDetails,
        metadata: Arc<Metadata>,
    ) {
        let gas_cost = metadata.get_gas_price_usd(gas_details.gas_paid(), self.utils.quote);

        cex_dex.adjust_for_gas_cost(&gas_cost);

        cex_dex.per_exchange_pnl.retain(|entry| entry.is_some());

        cex_dex.per_exchange_pnl.sort_by(|a, b| {
            b.as_ref()
                .unwrap()
                .aggregate_pnl
                .maker_taker_mid
                .1
                .cmp(&a.as_ref().unwrap().aggregate_pnl.maker_taker_mid.1)
        });
    }

    /// Filters and validates identified CEX-DEX arbitrage opportunities to
    /// minimize false positives.
    ///
    /// # Arguments
    /// * `possible_cex_dex` - The arbitrage opportunity being validated.
    /// * `info` - Transaction info providing additional context for validation.
    ///
    /// # Returns
    /// An option containing `BundleData::CexDex` if a valid opportunity is
    /// identified, otherwise `None`.
    fn filter_possible_cex_dex(
        &self,
        possible_cex_dex: CexDexProcessing,
        info: &TxInfo,
    ) -> Option<(f64, BundleData)> {
        let sanity_check_arb = possible_cex_dex.arb_sanity_check();
        let is_profitable_outlier = sanity_check_arb.is_profitable_outlier();

        let is_cex_dex_bot_with_significant_activity =
            info.is_searcher_of_type_with_count_threshold(MevType::CexDex, FILTER_THRESHOLD * 2);
        let is_labelled_cex_dex_bot = info.is_labelled_searcher_of_type(MevType::CexDex);

        let is_profitable_on_one_exchange = sanity_check_arb.profitable_exchanges_ask.len() == 1
            || sanity_check_arb.profitable_exchanges_mid.len() == 1;

        let should_include_based_on_pnl = sanity_check_arb.global_profitability.0
            || sanity_check_arb.global_profitability.1
            || sanity_check_arb.profitable_exchanges_ask.len() > 2
            || sanity_check_arb.profitable_exchanges_mid.len() > 2;

        let is_outlier_but_not_stable_swaps =
            is_profitable_outlier && !sanity_check_arb.is_stable_swaps;

        let is_profitable_one_exchange_but_not_stable_swaps =
            is_profitable_on_one_exchange && !sanity_check_arb.is_stable_swaps;

        let tx_attributes_meet_cex_dex_criteria = !info.is_classified
            && info.is_private
            && (info.is_searcher_of_type_with_count_threshold(MevType::CexDex, FILTER_THRESHOLD)
                || info
                    .contract_type
                    .as_ref()
                    .map_or(false, |contract_type| contract_type.could_be_mev_contract()));

        let is_cex_dex_based_on_historical_activity =
            is_cex_dex_bot_with_significant_activity || is_labelled_cex_dex_bot;

        if should_include_based_on_pnl
            || is_cex_dex_based_on_historical_activity
            || tx_attributes_meet_cex_dex_criteria
            || is_profitable_one_exchange_but_not_stable_swaps
            || is_outlier_but_not_stable_swaps
        {
            possible_cex_dex.into_bundle(info)
        } else {
            None
        }
    }

    /// Filters out triangular arbitrage
    pub fn is_triangular_arb(&self, dex_swaps: &[NormalizedSwap]) -> bool {
        // Not enough swaps to form a cycle, thus cannot be an atomic triangular
        // arbitrage.
        if dex_swaps.len() < 2 {
            return false
        }

        let original_token = dex_swaps[0].token_in.address;
        let final_token = dex_swaps.last().unwrap().token_out.address;

        original_token == final_token
    }
}

#[cfg(test)]
mod tests {

    use alloy_primitives::hex;
    use brontes_types::constants::{USDT_ADDRESS, WBTC_ADDRESS, WETH_ADDRESS};

    use crate::{
        test_utils::{InspectorTestUtils, InspectorTxRunConfig},
        Inspectors,
    };

    //TODO: Joe I am changing this for now because your quotes data seems to still
    // be incorrect. Please fix it, the previous value was 6772.69
    #[brontes_macros::test]
    async fn test_cex_dex() {
        let inspector_util = InspectorTestUtils::new(USDT_ADDRESS, 0.5).await;

        let tx = hex!("21b129d221a4f169de0fc391fe0382dbde797b69300a9a68143487c54d620295").into();

        let config = InspectorTxRunConfig::new(Inspectors::CexDex)
            .with_mev_tx_hashes(vec![tx])
            .with_expected_profit_usd(7054.49)
            .with_gas_paid_usd(78711.5);

        inspector_util.run_inspector(config, None).await.unwrap();
    }
    //TODO: Joe I am changing this for now because your quotes data seems to still
    // be incorrect. Please fix it, the previous value was 7201.40
    #[brontes_macros::test]
    async fn test_eoa_cex_dex() {
        let inspector_util = InspectorTestUtils::new(USDT_ADDRESS, 0.5).await;

        let tx = hex!("dfe3152caaf92e5a9428827ea94eff2a822ddcb22129499da4d5b6942a7f203e").into();

        let config = InspectorTxRunConfig::new(Inspectors::CexDex)
            .with_mev_tx_hashes(vec![tx])
            .with_expected_profit_usd(9595.80)
            .with_gas_paid_usd(6238.738);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_not_triangular_arb_false_positive() {
        let inspector_util = InspectorTestUtils::new(USDT_ADDRESS, 0.5).await;

        let tx = hex!("3329c54fef27a24cef640fbb28f11d3618c63662bccc4a8c5a0d53d13267652f").into();

        let config = InspectorTxRunConfig::new(Inspectors::CexDex)
            .with_mev_tx_hashes(vec![tx])
            .needs_tokens(vec![WETH_ADDRESS, WBTC_ADDRESS]);

        inspector_util.assert_no_mev(config).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_not_triangular_arb_false_positive_simple() {
        let inspector_util = InspectorTestUtils::new(USDT_ADDRESS, 0.5).await;

        let tx = hex!("31a1572dad67e949cff13d6ede0810678f25a30c6a3c67424453133bb822bd26").into();

        let config = InspectorTxRunConfig::new(Inspectors::CexDex)
            .with_mev_tx_hashes(vec![tx])
            .needs_token(hex!("aa7a9ca87d3694b5755f213b5d04094b8d0f0a6f").into());

        inspector_util.assert_no_mev(config).await.unwrap();
    }
}
