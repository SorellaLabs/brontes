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

use alloy_primitives::{Address, TxHash};
use brontes_database::libmdbx::LibmdbxReader;
use brontes_metrics::inspectors::OutlierMetrics;
use brontes_types::{
    db::cex::{quotes::FeeAdjustedQuote, CexExchange},
    display::utils::format_etherscan_url,
    mev::{Bundle, BundleData, MevType},
    normalized_actions::{
        accounting::ActionAccounting, Action, NormalizedSwap, NormalizedTransfer,
    },
    pair::Pair,
    tree::{BlockTree, GasDetails},
    BlockData, FastHashMap, FastHashSet, MultiBlockData, ToFloatNearest, TreeCollector,
    TreeSearchBuilder, TxInfo,
};
use malachite::{
    num::{
        arithmetic::traits::Reciprocal,
        basic::traits::{Two, Zero},
    },
    Rational,
};
use tracing::{debug, trace};

use super::types::{
    log_cex_dex_quote_delta, CexDexProcessing, ExchangeLeg, ExchangeLegCexPrice, PossibleCexDex,
};

pub const FILTER_THRESHOLD: u64 = 20;

use itertools::Itertools;

use crate::{shared_utils::SharedInspectorUtils, Inspector, Metadata};
pub struct CexDexQuotesInspector<'db, DB: LibmdbxReader> {
    utils:                SharedInspectorUtils<'db, DB>,
    _quotes_fetch_offset: u64,
    _cex_exchanges:       Vec<CexExchange>,
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
        quotes_fetch_offset: u64,
        metrics: Option<OutlierMetrics>,
    ) -> Self {
        Self {
            utils:                SharedInspectorUtils::new(quote, db, metrics),
            _quotes_fetch_offset: quotes_fetch_offset,
            _cex_exchanges:       cex_exchanges.to_owned(),
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

    fn inspect_block(&self, data: MultiBlockData) -> Self::Result {
        let block = data.get_most_recent_block();
        let BlockData { metadata, tree } = block;

        if metadata.cex_quotes.quotes.is_empty() {
            tracing::warn!("no cex quotes for this block");
            return vec![]
        }

        self.utils
            .get_metrics()
            .map(|m| {
                m.run_inspector(MevType::CexDexQuotes, || {
                    self.inspect_block_inner(tree.clone(), metadata.clone())
                })
            })
            .unwrap_or_else(|| self.inspect_block_inner(tree.clone(), metadata.clone()))
    }
}

impl<DB: LibmdbxReader> CexDexQuotesInspector<'_, DB> {
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
    fn inspect_block_inner(
        &self,
        tree: Arc<BlockTree<Action>>,
        metadata: Arc<Metadata>,
    ) -> Vec<Bundle> {
        metadata.display_pairs_quotes(self.utils.db);

        tree.clone()
            .collect_all(TreeSearchBuilder::default().with_actions([
                Action::is_swap,
                Action::is_transfer,
                Action::is_eth_transfer,
                Action::is_aggregator,
            ]))
            .filter_map(|(tx, swaps)| {
                let tx_info = tree.get_tx_info(tx, self.utils.db)?;

                // Return early if this is an defi automation contract
                if let Some(contract_type) = tx_info.contract_type.as_ref() {
                    if contract_type.is_defi_automation() {
                        return None
                    }
                }

                let deltas = swaps
                    .clone()
                    .into_iter()
                    .chain(
                        tx_info
                            .get_total_eth_value()
                            .iter()
                            .cloned()
                            .map(Action::from),
                    )
                    .account_for_actions();

                let (mut dex_swaps, rem): (Vec<_>, _) = self
                    .utils
                    .flatten_nested_actions(swaps.into_iter(), &|action| action.is_swap())
                    .split_return_rem(Action::try_swaps_merged);

                let transfers: Vec<_> = rem.into_iter().split_actions(Action::try_transfer);

                // robust way & deduplicate the swaps post
                if dex_swaps.is_empty() {
                    if let Some(extra) = self.try_convert_transfer_to_swap(transfers, &tx_info) {
                        dex_swaps.push(extra);
                    }
                }

                if dex_swaps.is_empty() {
                    trace!(    target: "brontes::cex-dex-quotes",
                "no dex swaps found\n Tx: {}", format_etherscan_url(&tx_info.tx_hash));
                    return None
                }

                if self.is_triangular_arb(&dex_swaps) {
                    trace!(
                        target: "brontes::cex-dex-markout",
                        "Filtered out CexDex because it is a triangular arb\n Tx: {}",
                        format_etherscan_url(&tx_info.tx_hash)
                    );
                    self.utils.get_metrics().inspect(|m| {
                        m.branch_filtering_trigger(MevType::CexDexQuotes, "is_triangular_arb")
                    });

                    return None
                }

                let mut possible_cex_dex: CexDexProcessing =
                    self.detect_cex_dex(dex_swaps, &metadata, &tx_info.tx_hash)?;

                self.gas_accounting(&mut possible_cex_dex, &tx_info.gas_details, metadata.clone());

                let price_map = possible_cex_dex.pnl.trade_prices.clone().into_iter().fold(
                    FastHashMap::default(),
                    |mut acc, x| {
                        acc.insert(x.token0, x.price0);
                        acc.insert(x.token1, x.price1);
                        acc
                    },
                );

                let (profit_usd, cex_dex) =
                    self.filter_possible_cex_dex(possible_cex_dex, &tx_info, &metadata)?;

                let header = self.utils.build_bundle_header(
                    vec![deltas],
                    vec![tx_info.tx_hash],
                    &tx_info,
                    profit_usd,
                    &[tx_info.gas_details],
                    metadata.clone(),
                    MevType::CexDexQuotes,
                    false,
                    |_, token, amount| Some(price_map.get(&token)? * amount),
                );

                Some(Bundle { header, data: cex_dex })
            })
            .collect::<Vec<_>>()
    }

    pub fn detect_cex_dex(
        &self,
        dex_swaps: Vec<NormalizedSwap>,
        metadata: &Metadata,
        tx_hash: &TxHash,
    ) -> Option<CexDexProcessing> {
        //TODO: Add smiths map to query most liquid dex for given pair
        let swaps = self.merge_possible_swaps(dex_swaps);

        let quotes = self.cex_quotes_for_swap(&swaps, metadata, 0, None);
        let cex_dex = self.detect_cex_dex_opportunity(&swaps, quotes, metadata, tx_hash)?;
        let cex_dex_processing = CexDexProcessing { dex_swaps: swaps, pnl: cex_dex };
        Some(cex_dex_processing)
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
        swap: &NormalizedSwap,
        cex_quote: FeeAdjustedQuote,
        metadata: &Metadata,
        tx_hash: &TxHash,
    ) -> Option<(ExchangeLeg, ExchangeLegCexPrice)> {
        let maker_taker_mid = cex_quote.maker_taker_mid();

        let output_of_cex_trade_maker = &maker_taker_mid.0 * &swap.amount_out;

        let smaller = min(&swap.amount_in, &output_of_cex_trade_maker);
        let larger = max(&swap.amount_in, &output_of_cex_trade_maker);

        if smaller * Rational::TWO < *larger {
            log_cex_dex_quote_delta(
                &tx_hash.to_string(),
                swap.token_in_symbol(),
                swap.token_out_symbol(),
                &cex_quote.exchange,
                swap.swap_rate().clone().to_float(),
                cex_quote.price_maker.0.clone().to_float(),
                &swap.token_in.address,
                &swap.token_out.address,
                &swap.amount_in,
                &swap.amount_out,
                &output_of_cex_trade_maker,
            );
            return None
        }

        // A positive amount indicates potential profit from selling the token in on the
        // DEX and buying it on the CEX.
        let maker_token_delta = &output_of_cex_trade_maker - &swap.amount_in;

        let token_price = metadata
            .cex_quotes
            .get_quote_from_most_liquid_exchange(
                &Pair(swap.token_in.address, self.utils.quote),
                metadata.microseconds_block_timestamp(),
                None,
            )?
            .maker_taker_mid()
            .0;

        // Amount * base_to_quote = USDT amount
        let base_to_quote = if token_price == Rational::ZERO {
            trace!("Token price is zero");
            return None
        } else {
            token_price.clone().reciprocal()
        };

        let pairs_price = ExchangeLegCexPrice {
            token0: swap.token_in.address,
            price0: base_to_quote.clone(),
            token1: swap.token_out.address,
            price1: (&token_price * maker_taker_mid.0.clone().reciprocal()).reciprocal(),
        };

        let pnl_mid = &maker_token_delta * &base_to_quote;

        Some((
            ExchangeLeg {
                pnl:           pnl_mid.to_float(),
                cex_mid_price: maker_taker_mid.0.to_float(),
                exchange:      CexExchange::Binance,
            },
            pairs_price,
        ))
    }

    /// Retrieves CEX quotes for a DEX swap, analyzing both direct and
    /// intermediary token pathways.
    fn cex_quotes_for_swap(
        &self,
        dex_swaps: &[NormalizedSwap],
        metadata: &Metadata,
        time_delta: u64,
        max_time_diff: Option<u64>,
    ) -> Vec<Option<FeeAdjustedQuote>> {
        dex_swaps
            .iter()
            .map(|dex_swap| {
                let pair = Pair(dex_swap.token_in.address, dex_swap.token_out.address);

                metadata
                    .cex_quotes
                    .get_quote_from_most_liquid_exchange(
                        &pair,
                        metadata.microseconds_block_timestamp() + (time_delta * 1_000_000),
                        max_time_diff,
                    )
                    .or_else(|| {
                        debug!(
                            "No CEX quote found for pair: {}-{}",
                            dex_swap.token_in_symbol(),
                            dex_swap.token_out_symbol(),
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

        cex_dex.pnl.adjust_for_gas_cost(gas_cost);
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
        metadata: &Metadata,
    ) -> Option<(f64, BundleData)> {
        tracing::info!(?possible_cex_dex, "filter time");
        let is_cex_dex_bot_with_significant_activity =
            info.is_searcher_of_type_with_count_threshold(MevType::CexDexQuotes, FILTER_THRESHOLD);
        let is_labelled_cex_dex_bot = info.is_labelled_searcher_of_type(MevType::CexDexQuotes);

        let should_include_based_on_pnl = possible_cex_dex.pnl.aggregate_pnl > 1.5;

        let should_include_if_know_cex_dex = possible_cex_dex.pnl.aggregate_pnl > 0.0;

        let is_cex_dex_based_on_historical_activity = (is_cex_dex_bot_with_significant_activity
            || is_labelled_cex_dex_bot)
            && should_include_if_know_cex_dex;

        if is_cex_dex_based_on_historical_activity || should_include_based_on_pnl {
            let t2 = self
                .cex_quotes_for_swap(&possible_cex_dex.dex_swaps, metadata, 2, None)
                .into_iter()
                .map(|quote_option| {
                    quote_option.map_or(0.0, |quote| quote.maker_taker_mid().0.to_float())
                })
                .collect_vec();

            let t12 = self
                .cex_quotes_for_swap(&possible_cex_dex.dex_swaps, metadata, 12, Some(500_000))
                .into_iter()
                .map(|quote_option| {
                    quote_option.map_or(0.0, |quote| quote.maker_taker_mid().0.to_float())
                })
                .collect_vec();

            let t30 = self
                .cex_quotes_for_swap(&possible_cex_dex.dex_swaps, metadata, 30, Some(2_000_000))
                .into_iter()
                .map(|quote_option| {
                    quote_option.map_or(0.0, |quote| quote.maker_taker_mid().0.to_float())
                })
                .collect_vec();

            let t60 = self
                .cex_quotes_for_swap(&possible_cex_dex.dex_swaps, metadata, 60, Some(4_000_000))
                .into_iter()
                .map(|quote_option| {
                    quote_option.map_or(0.0, |quote| quote.maker_taker_mid().0.to_float())
                })
                .collect_vec();

            let t300 = self
                .cex_quotes_for_swap(&possible_cex_dex.dex_swaps, metadata, 300, Some(15_000_000))
                .into_iter()
                .map(|quote_option| {
                    quote_option.map_or(0.0, |quote| quote.maker_taker_mid().0.to_float())
                })
                .collect_vec();

            possible_cex_dex.into_bundle(info, metadata.block_timestamp, t2, t12, t30, t60, t300)
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

    fn try_convert_transfer_to_swap(
        &self,
        mut transfers: Vec<NormalizedTransfer>,
        info: &TxInfo,
    ) -> Option<NormalizedSwap> {
        if !(transfers.len() == 2 && info.is_labelled_searcher_of_type(MevType::CexDexQuotes)) {
            return None
        }

        let t0 = transfers.remove(0);
        let t1 = transfers.remove(0);

        if t0.to == t1.from && Some(t0.to) != info.mev_contract {
            Some(NormalizedSwap {
                trace_index: t0.trace_index,
                amount_out: t1.amount,
                token_out: t1.token,
                amount_in: t0.amount,
                token_in: t0.token,
                from: t0.from,
                pool: t0.to,
                recipient: t0.from,
                ..Default::default()
            })
        } else if t1.to == t0.from && Some(t1.to) != info.mev_contract {
            Some(NormalizedSwap {
                trace_index: t1.trace_index,
                amount_out: t0.amount,
                token_out: t0.token,
                amount_in: t1.amount,
                token_in: t1.token,
                from: t1.from,
                pool: t1.to,
                recipient: t1.from,
                ..Default::default()
            })
        } else {
            None
        }
    }

    fn merge_possible_swaps(&self, swaps: Vec<NormalizedSwap>) -> Vec<NormalizedSwap> {
        let mut matching: FastHashMap<_, Vec<_>> = FastHashMap::default();

        for swap in &swaps {
            matching
                .entry(swap.token_in.clone())
                .or_default()
                .push(swap);
            matching
                .entry(swap.token_out.clone())
                .or_default()
                .push(swap);
        }

        let mut res = vec![];
        let mut voided = FastHashSet::default();

        for (intermediary, swaps) in matching {
            res.extend(swaps.into_iter().combinations(2).filter_map(|mut swaps| {
                let s0 = swaps.remove(0);
                let s1 = swaps.remove(0);

                // if s0 is first hop
                if s0.token_out == intermediary
                    && s0.token_out == s1.token_in
                    && s0.amount_out == s1.amount_in
                {
                    voided.insert(s0.clone());
                    voided.insert(s1.clone());
                    Some(NormalizedSwap {
                        from: s0.from,
                        recipient: s1.recipient,
                        token_in: s0.token_in.clone(),
                        token_out: s1.token_out.clone(),
                        amount_in: s0.amount_in.clone(),
                        amount_out: s1.amount_out.clone(),
                        protocol: s0.protocol,
                        pool: s0.pool,
                        ..Default::default()
                    })
                } else if s0.token_in == s1.token_out && s0.amount_in == s1.amount_out {
                    voided.insert(s0.clone());
                    voided.insert(s1.clone());
                    Some(NormalizedSwap {
                        from: s1.from,
                        recipient: s0.recipient,
                        token_in: s1.token_in.clone(),
                        token_out: s0.token_out.clone(),
                        amount_in: s1.amount_in.clone(),
                        amount_out: s0.amount_out.clone(),
                        protocol: s1.protocol,
                        pool: s1.pool,
                        ..Default::default()
                    })
                } else {
                    None
                }
            }));
        }

        swaps
            .into_iter()
            .filter(|s| !voided.contains(s))
            .chain(res)
            .collect()
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

    #[brontes_macros::test]
    async fn test_cex_dex() {
        let inspector_util = InspectorTestUtils::new(USDT_ADDRESS, 10.5).await;

        let tx = hex!("21b129d221a4f169de0fc391fe0382dbde797b69300a9a68143487c54d620295").into();

        let config = InspectorTxRunConfig::new(Inspectors::CexDex)
            .with_mev_tx_hashes(vec![tx])
            .with_expected_profit_usd(5152.80)
            .with_gas_paid_usd(79071.87);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_eoa_cex_dex() {
        let inspector_util = InspectorTestUtils::new(USDT_ADDRESS, 10.5).await;

        let tx = hex!("dfe3152caaf92e5a9428827ea94eff2a822ddcb22129499da4d5b6942a7f203e").into();

        let config = InspectorTxRunConfig::new(Inspectors::CexDex)
            .with_mev_tx_hashes(vec![tx])
            .with_expected_profit_usd(4858.63)
            .with_gas_paid_usd(6267.29);

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
