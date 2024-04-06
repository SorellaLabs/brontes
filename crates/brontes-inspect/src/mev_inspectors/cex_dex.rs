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
    db::{cex::CexExchange, dex::PriceAt},
    mev::{Bundle, BundleData, CexDex, MevType, StatArbDetails, StatArbPnl},
    normalized_actions::{accounting::ActionAccounting, Actions, NormalizedSwap},
    pair::Pair,
    tree::{BlockTree, GasDetails},
    ActionIter, ToFloatNearest, TreeSearchBuilder, TxInfo,
};
use malachite::{
    num::basic::traits::{Two, Zero},
    Rational,
};
use reth_primitives::Address;
use tracing::debug;

pub const FILTER_THRESHOLD: u64 = 20;

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
                let possible_cex_dex_by_exchange: Vec<PossibleCexDexLeg> =
                    self.detect_cex_dex(swaps, &metadata)?;

                let possible_cex_dex = self.gas_accounting(
                    possible_cex_dex_by_exchange,
                    &tx_info.gas_details,
                    metadata.clone(),
                )?;

                let cex_dex =
                    self.filter_possible_cex_dex(&possible_cex_dex, &tx_info, metadata.clone())?;

                let header = self.utils.build_bundle_header(
                    vec![deltas],
                    vec![tx_info.tx_hash],
                    &tx_info,
                    possible_cex_dex.pnl.taker_profit.clone().to_float(),
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
    ) -> Option<Vec<PossibleCexDexLeg>> {
        swaps.into_iter().try_fold(Vec::new(), |mut acc, swap| {
            match self.detect_cex_dex_opportunity(swap, metadata) {
                Some(leg) => {
                    acc.push(leg);
                    Some(acc)
                }
                None => None,
            }
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
        swap: NormalizedSwap,
        metadata: &Metadata,
    ) -> Option<PossibleCexDexLeg> {
        let cex_prices = self.cex_quotes_for_swap(&swap, metadata)?;

        let possible_legs: Vec<ExchangeLeg> = cex_prices
            .into_iter()
            .filter_map(|(exchange, price, is_direct_pair)| {
                self.profit_classifier(&swap, (exchange, price, is_direct_pair), metadata)
            })
            .collect();

        Some(PossibleCexDexLeg { swap, possible_legs })
    }

    /// For a given swap & CEX quote, calculates the potential profit from
    /// buying on DEX and selling on CEX. This function also accounts for CEX
    /// trading fees.
    fn profit_classifier(
        &self,
        swap: &NormalizedSwap,
        exchange_cex_price: (CexExchange, Rational, bool),
        metadata: &Metadata,
    ) -> Option<ExchangeLeg> {
        // If the price difference between the DEX and CEX is greater than 10x then this
        // is likely a false positive resulting from incorrect price data
        let smaller = swap.swap_rate().min(exchange_cex_price.1.clone());
        let larger = swap.swap_rate().max(exchange_cex_price.1.clone());

        if smaller * Rational::from(3) < larger {
            tracing::info!(
                "Filtered out possible CEX-DEX due to significant price delta.\n Price delta \
                 between CEX '{}' with price '{}' and DEX '{}' with price '{}'",
                exchange_cex_price.0,
                exchange_cex_price.1,
                swap.protocol,
                swap.swap_rate()
            );
            return None;
        }

        // A positive delta indicates potential profit from buying on DEX
        // and selling on CEX.
        let delta_price = &exchange_cex_price.1 - swap.swap_rate();
        let fees = exchange_cex_price.0.fees();

        let token_price = metadata
            .cex_quotes
            .get_quote_direct_or_via_intermediary(
                &Pair(swap.token_in.address, self.utils.quote),
                &exchange_cex_price.0,
            )?
            .price
            .0;

        let (maker_profit, taker_profit) = if exchange_cex_price.2 {
            (
                (&delta_price * (&swap.amount_out - &swap.amount_out * fees.0)) * &token_price,
                (delta_price * (&swap.amount_out - &swap.amount_out * fees.1)) * &token_price,
            )
        } else {
            (
                // Indirect pair pays twice the fee
                (&delta_price * (&swap.amount_out - &swap.amount_out * fees.0 * Rational::TWO))
                    * &token_price,
                (delta_price * (&swap.amount_out - &swap.amount_out * fees.1 * Rational::TWO))
                    * &token_price,
            )
        };

        Some(ExchangeLeg {
            exchange:  exchange_cex_price.0,
            cex_price: exchange_cex_price.1,
            pnl:       StatArbPnl { maker_profit, taker_profit },
            is_direct: exchange_cex_price.2,
        })
    }

    /// Retrieves CEX quotes for a DEX swap, analyzing both direct and
    /// intermediary token pathways.
    ///
    /// It attempts to retrieve quotes for the pair of tokens involved in the
    /// swap from each CEX specified in the inspector's configuration. If a
    /// direct quote is unavailable for a given exchange, the function seeks
    /// a quote via an intermediary token.
    ///
    /// Direct quotes are marked as `true`, indicating a single trade. Indirect
    /// quotes are marked as `false`, indicating two trades are required to
    /// complete the swap on the CEX. This distinction is needed so we can
    /// account for CEX trading fees.
    fn cex_quotes_for_swap(
        &self,
        swap: &NormalizedSwap,
        metadata: &Metadata,
    ) -> Option<Vec<(CexExchange, Rational, bool)>> {
        let pair = Pair(swap.token_out.address, swap.token_in.address);
        let quotes = self
            .cex_exchanges
            .iter()
            .filter_map(|&exchange| {
                metadata
                    .cex_quotes
                    .get_quote(&pair, &exchange)
                    .map(|cex_quote| (exchange, cex_quote.price.0, true))
                    .or_else(|| {
                        metadata
                            .cex_quotes
                            .get_quote_via_intermediary(&pair, &exchange)
                            .map(|cex_quote| (exchange, cex_quote.price.0, false))
                    })
                    .or_else(|| {
                        debug!(
                            "No CEX quote found for pair: {}, {} at exchange: {:?}",
                            swap.token_in, swap.token_out, exchange
                        );
                        None
                    })
            })
            .collect::<Vec<_>>();

        if quotes.is_empty() {
            None
        } else {
            debug!("CEX quotes found for pair: {}, {} at exchanges: {:?}", pair.0, pair.1, quotes);
            Some(quotes)
        }
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
        swaps_with_profit_by_exchange: Vec<PossibleCexDexLeg>,
        gas_details: &GasDetails,
        metadata: Arc<Metadata>,
    ) -> Option<PossibleCexDex> {
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
        metadata: Arc<Metadata>,
    ) -> Option<BundleData> {
        if self.is_triangular_arb(possible_cex_dex, info, metadata) {
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
    }

    /// Filters out triangular arbitrage
    pub fn is_triangular_arb(
        &self,
        possible_cex_dex: &PossibleCexDex,
        tx_info: &TxInfo,
        metadata: Arc<Metadata>,
    ) -> bool {
        // Not enough swaps to form a cycle, thus cannot be arbitrage.
        if possible_cex_dex.swaps.len() < 2 {
            return false
        }

        let original_token = possible_cex_dex.swaps[0].token_in.address;
        let final_token = possible_cex_dex.swaps.last().unwrap().token_out.address;

        // Check if there is a cycle
        if original_token != final_token {
            return false
        }
        let deltas = possible_cex_dex
            .swaps
            .clone()
            .into_iter()
            .map(Actions::from)
            .account_for_actions();

        let addr_usd_deltas = self
            .utils
            .usd_delta_by_address(
                tx_info.tx_index,
                PriceAt::Average,
                &deltas,
                metadata.clone(),
                false,
            )
            .unwrap_or_default();

        let profit = addr_usd_deltas
            .values()
            .fold(Rational::ZERO, |acc, delta| acc + delta);

        profit - metadata.get_gas_price_usd(tx_info.gas_details.gas_paid(), self.utils.quote)
            > Rational::ZERO
    }
}

#[derive(Debug)]
pub struct PossibleCexDex {
    pub swaps:       Vec<NormalizedSwap>,
    pub arb_details: Vec<StatArbDetails>,
    pub gas_details: GasDetails,
    pub pnl:         StatArbPnl,
}

impl PossibleCexDex {
    pub fn get_swaps(&self) -> Vec<Actions> {
        self.swaps
            .iter()
            .map(|s| Actions::Swap(s.clone()))
            .collect()
    }

    pub fn build_cex_dex_type(&self, info: &TxInfo) -> BundleData {
        BundleData::CexDex(CexDex {
            tx_hash:          info.tx_hash,
            gas_details:      self.gas_details,
            swaps:            self.swaps.clone(),
            stat_arb_details: self.arb_details.clone(),
            pnl:              self.pnl.clone(),
        })
    }
}

#[derive(Debug)]
pub struct PossibleCexDexLeg {
    pub swap:          NormalizedSwap,
    pub possible_legs: Vec<ExchangeLeg>,
}

/// Filters the most profitable exchange to execute the arbitrage on from a set
/// of potential exchanges for a given swap.
impl PossibleCexDexLeg {
    pub fn filter_most_profitable_leg(&self) -> Option<ExchangeLeg> {
        self.possible_legs
            .iter()
            .max_by_key(|leg| &leg.pnl.taker_profit)
            .cloned()
    }
}
#[derive(Clone, Debug)]
pub struct ExchangeLeg {
    pub exchange:  CexExchange,
    pub cex_price: Rational,
    pub pnl:       StatArbPnl,
    pub is_direct: bool,
}

#[cfg(test)]
mod tests {

    use alloy_primitives::hex;
    use brontes_types::constants::{USDT_ADDRESS, WBTC_ADDRESS, WETH_ADDRESS};

    use crate::{
        test_utils::{InspectorTestUtils, InspectorTxRunConfig},
        Inspectors,
    };

    #[brontes_macros::test]
    async fn test_cex_dex() {
        let inspector_util = InspectorTestUtils::new(USDT_ADDRESS, 0.5).await;

        let tx = hex!("21b129d221a4f169de0fc391fe0382dbde797b69300a9a68143487c54d620295").into();

        let config = InspectorTxRunConfig::new(Inspectors::CexDex)
            .with_mev_tx_hashes(vec![tx])
            .with_expected_profit_usd(6772.69)
            .with_gas_paid_usd(78993.39);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_eoa_cex_dex() {
        let inspector_util = InspectorTestUtils::new(USDT_ADDRESS, 0.5).await;

        let tx = hex!("dfe3152caaf92e5a9428827ea94eff2a822ddcb22129499da4d5b6942a7f203e").into();

        let config = InspectorTxRunConfig::new(Inspectors::CexDex)
            .with_mev_tx_hashes(vec![tx])
            .with_expected_profit_usd(7201.40)
            .with_gas_paid_usd(6261.08);

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
