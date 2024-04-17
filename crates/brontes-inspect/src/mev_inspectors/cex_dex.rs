//! This module implements the `CexDexInspector`, a specialized inspector
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
//! The `CexDexInspector` systematically identifies arbitrage opportunities
//! between CEXs and DEXs by analyzing transactions containing swap actions.
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

use std::sync::Arc;

use brontes_database::libmdbx::LibmdbxReader;
use brontes_types::{
    db::{
        cex::{CexExchange, FeeAdjustedQuote},
        dex::PriceAt,
    },
    mev::{ArbDetails, ArbPnl, Bundle, BundleData, CexDex, MevType},
    normalized_actions::{accounting::ActionAccounting, Actions, NormalizedSwap},
    pair::Pair,
    tree::{BlockTree, GasDetails},
    ActionIter, ToFloatNearest, TreeSearchBuilder, TxInfo,
};
use itertools::izip;
use malachite::{
    num::basic::traits::{Two, Zero},
    Rational,
};
use reth_primitives::Address;
use tracing::{debug, error};

pub const FILTER_THRESHOLD: u64 = 20;

use itertools::Itertools;

use crate::{shared_utils::SharedInspectorUtils, Inspector, Metadata};
pub struct CexDexInspector<'db, DB: LibmdbxReader> {
    utils:         SharedInspectorUtils<'db, DB>,
    cex_exchanges: Vec<CexExchange>,
}

impl<'db, DB: LibmdbxReader> CexDexInspector<'db, DB> {
    /// Constructs a new `CexDexInspector`.
    ///
    /// # Arguments
    ///
    /// * `quote` - The address of the quote asset
    /// * `db` - Database reader to our local libmdbx database
    /// * `cex_exchanges` - List of centralized exchanges to consider for
    ///   arbitrage.
    pub fn new(quote: Address, db: &'db DB, cex_exchanges: &[CexExchange]) -> Self {
        Self {
            utils:         SharedInspectorUtils::new(quote, db),
            cex_exchanges: cex_exchanges.to_owned(),
        }
    }
}

// TODO: Instead of doing instantaneous highest quote on single exchange
//TODO: Take all quotes 0 +.5 block time and weight by exchange + by best bid &
// ask amount

impl<DB: LibmdbxReader> Inspector for CexDexInspector<'_, DB> {
    type Result = Vec<Bundle>;

    fn get_id(&self) -> &str {
        "CexDex"
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
    fn process_tree(&self, tree: Arc<BlockTree<Actions>>, metadata: Arc<Metadata>) -> Self::Result {
        let swap_txes = tree
            .clone()
            .collect_all(TreeSearchBuilder::default().with_actions([
                Actions::is_swap,
                Actions::is_transfer,
                Actions::is_eth_transfer,
                Actions::is_aggregator,
            ]));

        swap_txes
            .filter_map(|(tx, swaps)| {
                let tx_info = tree.get_tx_info(tx, self.utils.db)?;

                // Return early if the tx is a solver settling trades
                if let Some(contract_type) = tx_info.contract_type.as_ref() {
                    if contract_type.is_solver_settlement() || contract_type.is_defi_automation() {
                        return None
                    }
                }

                let deltas = swaps.clone().into_iter().account_for_actions();
                let swaps = swaps
                    .into_iter()
                    .collect_action_vec(Actions::try_swaps_merged);

                // For each swap in the transaction, detect potential CEX-DEX
                let let_possible_cex_dex: CexDexProcessing =
                    self.detect_cex_dex(swaps, &metadata)?;

                let possible_cex_dex = self.gas_accounting(
                    possible_cex_dex_by_exchange,
                    &tx_info.gas_details,
                    metadata.clone(),
                )?;

                let cex_dex = self.filter_possible_cex_dex(&possible_cex_dex, &tx_info)?;

                let header = self.utils.build_bundle_header(
                    vec![deltas],
                    vec![tx_info.tx_hash],
                    &tx_info,
                    possible_cex_dex
                        .aggregate_pnl
                        .maker_taker_mid
                        .1
                        .clone()
                        .to_float(),
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

impl<DB: LibmdbxReader> CexDexInspector<'_, DB> {
    pub fn detect_cex_dex(
        &self,
        swaps: Vec<NormalizedSwap>,
        metadata: &Metadata,
    ) -> Option<CexDexProcessing> {
        let mut quotes = Vec::new();

        self.cex_exchanges.iter().map(|exchange| {
            quotes.push(self.cex_quotes_for_swap(&swaps, metadata));
        });

        let quotes_vwam: Vec<Option<FeeAdjustedQuote>> = izip!(quotes)
            .enumerate()
            .map(|(index, row)| {
                let some_quotes: Vec<FeeAdjustedQuote> =
                    row.into_iter().filter_map(|quote| quote.clone()).collect();
                if some_quotes.is_empty() {
                    None
                } else {
                    let volume_weighted_quote = metadata
                        .cex_quotes
                        .get_volume_weighted_quote(&some_quotes, &dex_swap[index]);
                    Some(volume_weighted_quote)
                }
            })
            .collect();

        let global_vwam_cex_dex = self.detect_cex_dex_opportunity(&swaps, quotes_vwam, metadata);

        let per_exchange_pnl = quotes
            .into_iter()
            .map(|quotes| self.detect_cex_dex_opportunity(&swaps, quotes, metadata))
            .collect();

        Some(CexDexProcessing {
            swaps,
            global_vmam_cex_dex: Some(global_vwam_cex_dex),
            per_exchange_pnl,
        })
    }

    /// Detects potential CEX-DEX arbitrage opportunities for a given swap.
    ///
    /// # Arguments
    ///
    /// * `swap` - The swap action to analyze.
    /// * `metadata` - Combined metadata for additional context in analysis.
    ///
    /// # Returns
    ///
    /// An option containing a `PossibleCexDexLeg` if an opportunity is found,
    /// otherwise `None`.
    pub fn detect_cex_dex_opportunity(
        &self,
        swaps: &[NormalizedSwap],
        cex_prices: Vec<Option<FeeAdjustedQuote>>,
        metadata: &Metadata,
    ) -> Option<PossibleCexDex> {
        let exchange_legs = swaps
            .iter()
            .zip(cex_prices.into_iter())
            .filter_map(|(swap, quote)| {
                if let Some(q) = quote {
                    self.profit_classifier(swap, &q, metadata)
                } else {
                    None
                }
            })
            .collect_vec();

        PossibleCexDex::from_exchange_legs(exchange_legs)
    }

    /// For a given swap & CEX quote, calculates the potential profit from
    /// buying on DEX and selling on CEX.
    fn profit_classifier(
        &self,
        swap: &NormalizedSwap,
        cex_quote: &FeeAdjustedQuote,
        metadata: &Metadata,
    ) -> Option<ExchangeLeg> {
        // If the price difference between the DEX and CEX is greater than 2x, this
        // is likely a false positive resulting from incorrect price data
        let smaller = swap.swap_rate().min(cex_quote.price_maker.1.clone());
        let larger = swap.swap_rate().max(cex_quote.price_maker.1.clone());

        if smaller * Rational::from(2) < larger {
            error!(
                "\n\x1b[1;35mDetected significant price delta for direct pair for {} - {} on {}:\x1b[0m\n\
                 - \x1b[1;36mDEX Swap Rate:\x1b[0m {:.7}\n\
                 - \x1b[1;36mCEX Price:\x1b[0m {:.7}\n\
                 - Token Contracts:\n\
                   * Token In: https://etherscan.io/address/{}\n\
                   * Token Out: https://etherscan.io/address/{}",
                swap.token_in_symbol(),
                swap.token_out_symbol(),
                cex_quote.exchange,
                swap.swap_rate().to_float(),
                cex_quote.price_maker.1.to_float(),
                swap.token_in.address,
                swap.token_out.address
            );
            return None;
        }

        // A positive delta indicates potential profit from buying on DEX
        // and selling on CEX.
        let maker_taker_mid = cex_quote.maker_taker_mid();

        let maker_mid_delta = &maker_taker_mid.0 - swap.swap_rate();
        let taker_mid_delta = &maker_taker_mid.1 - swap.swap_rate();

        let token_price = metadata.cex_quotes.get_quote_direct_or_via_intermediary(
            &Pair(swap.token_in.address, self.utils.quote),
            &cex_quote.exchange,
            &swap,
        )?;

        let token_maker_taker_mid = token_price.maker_taker_mid();

        let pnl_mid = (
            &maker_mid_delta * &swap.amount_out * &token_maker_taker_mid.0,
            &taker_mid_delta * &swap.amount_out * &token_maker_taker_mid.1,
        );

        let maker_ask_delta = &cex_quote.price_maker.1 - swap.swap_rate();
        let taker_ask_delta = &cex_quote.price_taker.1 - swap.swap_rate();

        let token_maker_taker_ask = token_price.maker_taker_ask();

        let pnl_ask = (
            &maker_ask_delta * &swap.amount_out * &token_maker_taker_ask.0,
            &taker_ask_delta * &swap.amount_out * &token_maker_taker_ask.1,
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
    ) -> Vec<Option<FeeAdjustedQuote>> {
        dex_swaps
            .iter()
            .map(|swap| {
                let pair = Pair(swap.token_out.address, swap.token_in.address);
                let exchange = &metadata.exchange; // Assuming 'exchange' is part of 'metadata'

                metadata
                    .cex_quotes
                    .get_quote_direct_or_via_intermediary(&pair, exchange, &swap)
                    .or_else(|| {
                        debug!(
                            "No CEX quote found for pair: {}, {} at exchange: {:?}",
                            swap.token_in_symbol(),
                            swap.token_out_symbol(),
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
    ///
    /// # Arguments
    /// * `swaps_with_profit_by_exchange` - A vector of `PossibleCexDexLeg`
    ///   instances to be analyzed.
    /// * `gas_details` - Details of the gas costs associated with the
    ///   transaction.
    /// * `metadata` - Shared metadata providing additional context and price
    ///   data.
    ///
    /// # Returns
    /// A `PossibleCexDex` instance representing the finalized arbitrage
    /// opportunity after accounting for gas costs.

    fn gas_accounting(
        &self,
        swaps_with_profit_by_exchange: CexDexProcessing,
        gas_details: &GasDetails,
        metadata: Arc<Metadata>,
    ) -> Option<PossibleCexDex> {
        /*
            let mut swaps = Vec::new();
            let mut arb_details = Vec::new();
            let mut total_arb_pre_gas = StatArbPnl::default();

            swaps_with_profit_by_exchange
                .iter()
                .for_each(|swap_with_profit| {
                    if let Some(most_profitable_leg) = swap_with_profit.filter_most_profitable_leg() {
                        swaps.push(swap_with_profit.swap.clone());
                        arb_details.push(StatArbDetails {
                            cex_exchange: most_profitable_leg.exchange,
                            cex_price:    most_profitable_leg.cex_price,
                            dex_exchange: swap_with_profit.swap.protocol,
                            dex_price:    swap_with_profit.swap.swap_rate(),
                            pnl_pre_gas:  most_profitable_leg.pnl.clone(),
                        });
                        total_arb_pre_gas.maker_profit += most_profitable_leg.pnl.maker_profit;
                        total_arb_pre_gas.taker_profit += most_profitable_leg.pnl.taker_profit;
                    }
                });

            if swaps.is_empty() {
                return None
            }

            let gas_cost = metadata.get_gas_price_usd(gas_details.gas_paid(), self.utils.quote);

            let pnl = StatArbPnl {
                maker_profit: total_arb_pre_gas.maker_profit - gas_cost.clone(),
                taker_profit: total_arb_pre_gas.taker_profit - gas_cost,
            };

            Some(PossibleCexDex { swaps, arb_details, gas_details: *gas_details, pnl })
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
            possible_cex_dex: &PossibleCexDex,
            info: &TxInfo,
        ) -> Option<BundleData> {
            if self.is_triangular_arb(possible_cex_dex) {
                return None
            }

            let has_positive_pnl = possible_cex_dex.pnl.maker_profit > Rational::ZERO
                || possible_cex_dex.pnl.taker_profit > Rational::ZERO;

            if has_positive_pnl
                || (!info.is_classified
                    && (possible_cex_dex.gas_details.coinbase_transfer.is_some()
                        && info.is_private
                        && info.is_searcher_of_type_with_count_threshold(
                            MevType::CexDex,
                            FILTER_THRESHOLD,
                        )
                        || info.is_cex_dex_call))
                || info.is_searcher_of_type_with_count_threshold(MevType::CexDex, FILTER_THRESHOLD * 3)
                || info.is_labelled_searcher_of_type(MevType::CexDex)
            {
                Some(possible_cex_dex.build_cex_dex_type(info))
            } else {
                None
            }
            */
    }

    /// Filters out triangular arbitrage
    pub fn is_triangular_arb(&self, possible_cex_dex: &PossibleCexDex) -> bool {
        // Not enough swaps to form a cycle, thus cannot be arbitrage.
        if possible_cex_dex.swaps.len() < 2 {
            return false
        }

        let original_token = possible_cex_dex.swaps[0].token_in.address;
        let final_token = possible_cex_dex.swaps.last().unwrap().token_out.address;

        original_token == final_token
    }
}

#[derive(Debug, Default)]
pub struct PossibleCexDex {
    pub quotes:        Vec<Option<FeeAdjustedQuote>>,
    pub pnl:           Vec<Option<ArbPnl>>,
    pub aggregate_pnl: ArbPnl,
}

impl PossibleCexDex {
    pub fn from_exchange_legs(exchange_legs: Vec<Option<ExchangeLeg>>) -> Option<Self> {
        let mut quotes = Vec::new();
        let mut pnls = Vec::new();

        let mut total_mid_maker = Rational::ZERO;
        let mut total_mid_taker = Rational::ZERO;
        let mut total_ask_maker = Rational::ZERO;
        let mut total_ask_taker = Rational::ZERO;

        for leg in exchange_legs {
            if let Some(leg) = leg {
                quotes.push(Some(leg.cex_quote.clone()));
                pnls.push(Some(leg.pnl.clone()));

                total_mid_maker += leg.pnl.maker_taker_mid.0;
                total_mid_taker += leg.pnl.maker_taker_mid.1;
                total_ask_maker += leg.pnl.maker_taker_ask.0;
                total_ask_taker += leg.pnl.maker_taker_ask.1;
            } else {
                quotes.push(None);
                pnls.push(None);
            }
        }

        let aggregate_pnl = ArbPnl {
            maker_taker_mid: (total_mid_maker, total_mid_taker),
            maker_taker_ask: (total_ask_maker, total_ask_taker),
        };

        PossibleCexDex { quotes, pnl: pnls, aggregate_pnl }
    }
}

impl PossibleCexDex {
    pub fn build_cex_dex_type(&self, info: &TxInfo) -> BundleData {
        todo!();
        /*
        BundleData::CexDex(CexDex {
            tx_hash:          info.tx_hash,
            gas_details:      self.gas_details,
            swaps:            self.swaps.clone(),
            stat_arb_details: self.arb_details.clone(),
            pnl:              self.pnl.clone(),
        })*/
    }
}

pub struct CexDexProcessing {
    pub swaps:               Vec<NormalizedSwap>,
    pub global_vmam_cex_dex: Option<PossibleCexDex>,
    pub per_exchange_pnl:    Vec<Option<PossibleCexDex>>,
}

#[derive(Debug)]
pub struct PossibleCexDexLeg {
    pub swap:          NormalizedSwap,
    pub possible_legs: ExchangeLeg,
}

#[derive(Clone, Debug)]
pub struct ExchangeLeg {
    pub cex_quote: FeeAdjustedQuote,
    pub pnl:       ArbPnl,
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
