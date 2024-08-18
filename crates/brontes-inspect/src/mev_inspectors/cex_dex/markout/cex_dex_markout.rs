use std::{
    cmp::{max, min},
    sync::Arc,
};

use alloy_primitives::FixedBytes;
use brontes_database::libmdbx::LibmdbxReader;
use brontes_metrics::inspectors::OutlierMetrics;
use brontes_types::{
    db::{
        cex::{
            trades::{
                config::CexDexTradeConfig,
                optimistic::OptimisticPrice,
                time_window_vwam::{ExchangePath, WindowExchangePrice},
            },
            CexExchange,
        },
        token_info::TokenInfoWithAddress,
    },
    display::utils::format_etherscan_url,
    mev::{Bundle, BundleData, MevType, OptimisticTrade},
    normalized_actions::{
        accounting::{ActionAccounting, AddressDeltas},
        Action, NormalizedBatch, NormalizedSwap,
    },
    pair::Pair,
    tree::{BlockTree, GasDetails},
    BlockData, FastHashMap, FastHashSet, MultiBlockData, ToFloatNearest, TreeCollector,
    TreeSearchBuilder, TxInfo,
};
use itertools::{multizip, Itertools};
use malachite::{
    num::{
        arithmetic::traits::Reciprocal,
        basic::traits::{One, Zero},
    },
    Rational,
};
use reth_primitives::Address;
use tracing::trace;

use super::{
    log_cex_trade_price_delta, ArbLeg, CexDexProcessing, CexPricesForSwaps, ExchangeLegCexPrice,
    OptimisticDetails, PossibleCexDex, PriceCalcType,
};

// The threshold for the number of CEX-DEX trades an address is required to make
// to classify a a negative pnl cex-dex trade as a CEX-DEX trade
pub const FILTER_THRESHOLD: u64 = 20;

use crate::{shared_utils::SharedInspectorUtils, Inspector, Metadata};

pub struct CexDexMarkoutInspector<'db, DB: LibmdbxReader> {
    pub utils:     SharedInspectorUtils<'db, DB>,
    trade_config:  CexDexTradeConfig,
    cex_exchanges: Vec<CexExchange>,
}

impl<'db, DB: LibmdbxReader> CexDexMarkoutInspector<'db, DB> {
    pub fn new(
        quote: Address,
        db: &'db DB,
        cex_exchanges: &[CexExchange],
        trade_config: CexDexTradeConfig,
        metrics: Option<OutlierMetrics>,
    ) -> Self {
        Self {
            utils: SharedInspectorUtils::new(quote, db, metrics),
            trade_config,
            cex_exchanges: cex_exchanges.to_owned(),
        }
    }
}

impl<DB: LibmdbxReader> Inspector for CexDexMarkoutInspector<'_, DB> {
    type Result = Vec<Bundle>;

    fn get_id(&self) -> &str {
        "CexDexMarkout"
    }

    fn get_quote_token(&self) -> Address {
        self.utils.quote
    }

    fn inspect_block(&self, data: MultiBlockData) -> Self::Result {
        let block = data.get_most_recent_block();
        let BlockData { metadata, tree } = block;

        if metadata.cex_trades.is_none() {
            tracing::error!("no cex trades for block: {}", block.metadata.block_num);
            return vec![]
        }

        self.utils
            .get_metrics()
            .map(|m| {
                m.run_inspector(MevType::CexDexTrades, || {
                    self.inspect_block_inner(tree.clone(), metadata.clone())
                })
            })
            .unwrap_or_else(|| self.inspect_block_inner(tree.clone(), metadata.clone()))
    }
}

impl<DB: LibmdbxReader> CexDexMarkoutInspector<'_, DB> {
    fn inspect_block_inner(
        &self,
        tree: Arc<BlockTree<Action>>,
        metadata: Arc<Metadata>,
    ) -> Vec<Bundle> {
        let (hashes, actions): (Vec<_>, Vec<_>) = tree
            .clone()
            .collect_all(TreeSearchBuilder::default().with_actions([
                Action::is_swap,
                Action::is_transfer,
                Action::is_eth_transfer,
                Action::is_aggregator,
                Action::is_batch,
            ]))
            .unzip();

        let tx_info = tree.get_tx_info_batch(&hashes, self.utils.db);

        multizip((actions, tx_info))
            .filter_map(|(actions, tx_info)| {
                let tx_info = tx_info?;
                if self.should_filter_tx(&tx_info) {
                    return None
                }

                if actions.iter().any(Action::is_batch) {
                    self.process_batch_swaps(actions, tx_info, metadata.clone())
                } else {
                    self.process_dex_swaps(actions, tx_info, metadata.clone())
                }
            })
            .collect()
    }

    fn should_filter_tx(&self, tx_info: &TxInfo) -> bool {
        if let Some(contract_type) = tx_info.contract_type.as_ref() {
            if contract_type.is_defi_automation() {
                trace!(
                    target: "brontes::cex-dex-markout",
                    "Filtered out CexDex tx because it is a contract of type {:?}\n Tx: {}",
                    contract_type,
                    format_etherscan_url(&tx_info.tx_hash)
                );
                self.utils.get_metrics().inspect(|m| {
                    m.branch_filtering_trigger(MevType::CexDexTrades, "is_defi_automation")
                });
                return true
            }
        }
        false
    }

    fn process_dex_swaps(
        &self,
        actions: Vec<Action>,
        tx_info: TxInfo,
        metadata: Arc<Metadata>,
    ) -> Option<Bundle> {
        let deltas = actions
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
            .flatten_nested_actions(actions.into_iter(), &|action| action.is_swap())
            .split_return_rem(Action::try_swaps_merged);

        let transfers: Vec<_> = rem.into_iter().split_actions(Action::try_transfer);

        if dex_swaps.is_empty() {
            if let Some(extra) = self.utils.cex_try_convert_transfer_to_swap(
                transfers,
                &tx_info,
                MevType::CexDexTrades,
            ) {
                dex_swaps.push(extra);
            }
        }

        if self.is_triangular_arb(&dex_swaps) {
            trace!(
                target: "brontes::cex-dex-markout",
                "Filtered out CexDex because it is a triangular arb\n Tx: {}",
                format_etherscan_url(&tx_info.tx_hash)
            );
            self.utils.get_metrics().inspect(|m| {
                m.branch_filtering_trigger(MevType::CexDexTrades, "is_triangular_arb")
            });
            return None
        }

        self.process_swaps(dex_swaps, tx_info, metadata, deltas, false)
    }

    fn process_batch_swaps(
        &self,
        actions: Vec<Action>,
        tx_info: TxInfo,
        metadata: Arc<Metadata>,
    ) -> Option<Bundle> {
        let deltas = actions
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

        let dex_swaps: Vec<_> = actions
            .into_iter()
            .filter_map(|action| match action {
                Action::Batch(NormalizedBatch { user_swaps, .. }) => Some(user_swaps),
                _ => None,
            })
            .flatten()
            .collect();

        self.process_swaps(dex_swaps, tx_info, metadata, deltas, true)
    }

    fn process_swaps(
        &self,
        dex_swaps: Vec<NormalizedSwap>,
        tx_info: TxInfo,
        metadata: Arc<Metadata>,
        deltas: AddressDeltas,
        batch_swap: bool,
    ) -> Option<Bundle> {
        if dex_swaps.is_empty() {
            trace!(
                target: "brontes::cex-dex-markout",
                "no dex swaps found\n Tx: {}",
                format_etherscan_url(&tx_info.tx_hash)
            );
            return None
        }

        let mut possible_cex_dex: CexDexProcessing = self.detect_cex_dex(
            dex_swaps,
            &metadata,
            tx_info.is_searcher_of_type(MevType::CexDexTrades)
                || tx_info.is_labelled_searcher_of_type(MevType::CexDexTrades)
                || tx_info.is_labelled_searcher_of_type(MevType::CexDexRfq)
                || tx_info.is_searcher_of_type(MevType::JitCexDex),
            &tx_info,
        )?;

        self.gas_accounting(&mut possible_cex_dex, &tx_info.gas_details, metadata.clone());

        let (profit_usd, cex_dex, trade_prices) =
            self.filter_possible_cex_dex(possible_cex_dex, &tx_info, metadata.clone())?;

        let price_map = trade_prices
            .into_iter()
            .fold(FastHashMap::default(), |mut acc, x| {
                acc.insert(x.token0, x.price0);
                acc.insert(x.token1, x.price1);
                acc
            });

        let header: brontes_types::mev::BundleHeader = self.utils.build_bundle_header(
            vec![deltas],
            vec![tx_info.tx_hash],
            &tx_info,
            profit_usd,
            &[tx_info.gas_details],
            metadata.clone(),
            if batch_swap { MevType::CexDexRfq } else { MevType::CexDexTrades },
            false,
            |_, token, amount| Some(price_map.get(&token)? * &amount),
        );

        Some(Bundle { header, data: cex_dex })
    }

    pub fn detect_cex_dex(
        &self,
        dex_swaps: Vec<NormalizedSwap>,
        metadata: &Metadata,
        marked_cex_dex: bool,
        tx_info: &TxInfo,
    ) -> Option<CexDexProcessing> {
        let cex_prices =
            self.cex_prices_for_swaps(dex_swaps, metadata, marked_cex_dex, tx_info.tx_hash);

        let merged_swaps = cex_prices.dex_swaps.clone();

        let global_vwam: Option<PossibleCexDex> =
            self.process_global_vwam(&cex_prices, metadata, tx_info);

        let per_exchange_pnl = self.process_per_exchange(&cex_prices, metadata, tx_info);

        let optimstic_res: Option<OptimisticDetails> =
            self.process_optimistic(cex_prices, metadata, tx_info);

        CexDexProcessing::new(merged_swaps, global_vwam, per_exchange_pnl, optimstic_res)
    }

    fn process_global_vwam(
        &self,
        cex_prices: &CexPricesForSwaps,
        metadata: &Metadata,
        tx_info: &TxInfo,
    ) -> Option<PossibleCexDex> {
        cex_prices.global_price().and_then(|global_prices| {
            PossibleCexDex::from_arb_legs(
                cex_prices
                    .dex_swaps
                    .iter()
                    .zip(global_prices)
                    .map(|(dex_swap, (global_price, pair))| {
                        self.profit_classifier(
                            dex_swap,
                            pair.to_vec(),
                            global_price,
                            CexExchange::VWAP,
                            metadata,
                            tx_info,
                            PriceCalcType::TimeWindowGlobal,
                        )
                    })
                    .collect(),
            )
        })
    }

    fn process_per_exchange(
        &self,
        cex_prices: &CexPricesForSwaps,
        metadata: &Metadata,
        tx_info: &TxInfo,
    ) -> Vec<Option<PossibleCexDex>> {
        cex_prices
            .per_exchange_trades(self.cex_exchanges.as_slice())
            .into_iter()
            .map(|(exchange, exchange_paths)| {
                let arb_legs: Vec<Option<ArbLeg>> = cex_prices
                    .dex_swaps
                    .iter()
                    .zip(exchange_paths)
                    .map(|(dex_swap, exchange_path)| {
                        exchange_path.and_then(|(path, pairs)| {
                            self.profit_classifier(
                                dex_swap,
                                pairs.to_vec(),
                                path,
                                *exchange,
                                metadata,
                                tx_info,
                                PriceCalcType::TimeWindowPerEx,
                            )
                        })
                    })
                    .collect();
                PossibleCexDex::from_arb_legs(arb_legs)
            })
            .collect()
    }

    //TODO: Remove horendous clones, just getting ouput for debugging purposes
    // right now
    pub fn process_optimistic(
        &self,
        cex_prices: CexPricesForSwaps,
        metadata: &Metadata,
        tx_info: &TxInfo,
    ) -> Option<OptimisticDetails> {
        let arb_legs_and_trades: Vec<(Option<ArbLeg>, Vec<OptimisticTrade>)> = cex_prices
            .dex_swaps
            .into_iter()
            .zip(cex_prices.optimistic)
            .map(|(dex_swap, opt_price)| {
                opt_price.map_or((None, Vec::new()), |price| {
                    let arb_leg = self.profit_classifier(
                        &dex_swap,
                        price.pairs.clone(),
                        &price.global,
                        CexExchange::OptimisticVWAP,
                        metadata,
                        tx_info,
                        PriceCalcType::Optimistic,
                    );
                    (arb_leg, price.trades_used)
                })
            })
            .collect();

        if arb_legs_and_trades.is_empty() {
            return None
        }

        let (arb_legs, trade_details): (Vec<_>, Vec<_>) = arb_legs_and_trades.into_iter().unzip();

        if arb_legs.iter().all(Option::is_none) {
            None
        } else {
            Some(OptimisticDetails::new(arb_legs, trade_details))
        }
    }

    /// For a given swap & CEX quote, calculates the potential profit from
    /// buying on DEX and selling on CEX.
    fn profit_classifier(
        &self,
        swap: &NormalizedSwap,
        pairs: Vec<Pair>,
        cex_quote: &ExchangePath,
        exchange: CexExchange,
        metadata: &Metadata,
        tx_info: &TxInfo,
        price_calculation_type: PriceCalcType,
    ) -> Option<ArbLeg> {
        let (output_of_cex_trade_maker, output_of_cex_trade_taker) =
            (&cex_quote.price_maker * &swap.amount_out, &cex_quote.price_taker * &swap.amount_out);

        // A positive amount indicates potential profit from selling the token in on the
        // DEX and buying it on the CEX.
        let maker_token_delta = &output_of_cex_trade_maker - &swap.amount_in;
        let taker_token_delta = &output_of_cex_trade_taker - &swap.amount_in;

        let vol = Rational::ONE;

        let pair = Pair(swap.token_in.address, self.utils.quote);

        let token_price = metadata
            .cex_trades
            .as_ref()
            .unwrap()
            .calculate_time_window_vwam(
                self.trade_config,
                &self.cex_exchanges,
                pair,
                &vol,
                metadata.microseconds_block_timestamp(),
                true,
                swap,
                tx_info.tx_hash,
            )?
            .global
            .price_maker;

        // Amount * base_to_quote = USDT amount
        let base_to_quote = if token_price == Rational::ZERO {
            trace!("Token price is zero");
            return None
        } else {
            token_price.clone().reciprocal()
        };

        if cex_quote.price_maker == Rational::ZERO {
            return None
        }

        let pairs_price = ExchangeLegCexPrice {
            token0: swap.token_in.address,
            price0: base_to_quote.clone(),
            token1: swap.token_out.address,
            price1: (&token_price * cex_quote.price_maker.clone().reciprocal()).reciprocal(),
        };

        let pnl = (&maker_token_delta * &base_to_quote, &taker_token_delta * &base_to_quote);

        let smaller = min(&swap.amount_in, &output_of_cex_trade_maker);
        let larger = max(&swap.amount_in, &output_of_cex_trade_maker);

        let max_diff = max_arb_delta(tx_info, &pnl.0);

        if smaller * max_diff < *larger {
            log_cex_trade_price_delta(
                &tx_info.tx_hash,
                swap.token_in_symbol(),
                swap.token_out_symbol(),
                swap.swap_rate().clone().to_float(),
                cex_quote.price_maker.clone().to_float(),
                &swap.token_in.address,
                &swap.token_out.address,
                price_calculation_type,
                &swap.amount_in,
                &swap.amount_out,
                &output_of_cex_trade_maker,
            );
            return None
        }

        Some(ArbLeg {
            price: cex_quote.clone(),
            pairs,
            exchange,
            pnl_maker: pnl.0,
            pnl_taker: pnl.1,
            token_price: pairs_price,
        })
    }

    fn cex_prices_for_swaps(
        &self,
        dex_swaps: Vec<NormalizedSwap>,
        metadata: &Metadata,
        marked_cex_dex: bool,
        tx_hash: FixedBytes<32>,
    ) -> CexPricesForSwaps {
        let merged_swaps = self.utils.cex_merge_possible_swaps(dex_swaps);

        let (time_window_vwam, optimistic): (Vec<_>, Vec<_>) = merged_swaps
            .clone()
            .iter()
            .filter(|swap| swap.amount_out != Rational::ZERO)
            .map(|swap| self.calculate_cex_price(swap, metadata, marked_cex_dex, tx_hash))
            .unzip();

        CexPricesForSwaps { dex_swaps: merged_swaps, time_window_vwam, optimistic }
    }

    fn calculate_cex_price(
        &self,
        swap: &NormalizedSwap,
        metadata: &Metadata,
        marked_cex_dex: bool,
        tx_hash: FixedBytes<32>,
    ) -> (Option<WindowExchangePrice>, Option<OptimisticPrice>) {
        let pair = Pair(swap.token_in.address, swap.token_out.address);

        let window_fn = || {
            metadata
                .cex_trades
                .as_ref()
                .unwrap()
                .calculate_time_window_vwam(
                    self.trade_config,
                    &self.cex_exchanges,
                    pair,
                    &swap.amount_out,
                    metadata.microseconds_block_timestamp(),
                    marked_cex_dex,
                    swap,
                    tx_hash,
                )
        };

        let window = self
            .utils
            .get_metrics()
            .map(|m| m.run_cex_price_window(window_fn))
            .unwrap_or_else(window_fn);

        let optimistic = || {
            metadata.cex_trades.as_ref().unwrap().get_optimistic_vmap(
                self.trade_config,
                &self.cex_exchanges,
                pair,
                &swap.amount_out,
                metadata.microseconds_block_timestamp(),
                None,
                marked_cex_dex,
                swap,
                tx_hash,
            )
        };

        let optimistic = self
            .utils
            .get_metrics()
            .map(|m| m.run_cex_price_vol(optimistic))
            .unwrap_or_else(optimistic);

        if (window.is_none() || optimistic.is_none()) && marked_cex_dex {
            self.utils
                .get_metrics()
                .inspect(|m| m.missing_cex_pair(pair));
        }

        (window, optimistic)
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

    pub fn gas_accounting(
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
                .aggregate_pnl_maker
                .cmp(&a.as_ref().unwrap().aggregate_pnl_maker)
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
    pub fn filter_possible_cex_dex(
        &self,
        possible_cex_dex: CexDexProcessing,
        info: &TxInfo,
        metadata: Arc<Metadata>,
    ) -> Option<(f64, BundleData, Vec<ExchangeLegCexPrice>)> {
        let sanity_check_arb = possible_cex_dex.arb_sanity_check();
        let is_profitable_outlier = sanity_check_arb.is_profitable_outlier();

        let is_cex_dex_bot_with_significant_activity = info
            .is_searcher_of_type_with_count_threshold(MevType::CexDexTrades, FILTER_THRESHOLD * 2);
        let is_labelled_cex_dex_bot = info.is_labelled_searcher_of_type(MevType::CexDexTrades);

        let is_profitable_on_one_exchange = sanity_check_arb.profitable_exchanges_maker.len() == 1
            || sanity_check_arb.profitable_exchanges_taker.len() == 1;

        let should_include_based_on_pnl = sanity_check_arb.global_profitability.0
            || sanity_check_arb.global_profitability.1
            || sanity_check_arb.profitable_exchanges_maker.len() > 2
            || sanity_check_arb.profitable_exchanges_taker.len() > 2;

        let is_outlier_but_not_stable_swaps =
            is_profitable_outlier && !sanity_check_arb.is_stable_swaps;

        let is_profitable_one_exchange_but_not_stable_swaps =
            is_profitable_on_one_exchange && !sanity_check_arb.is_stable_swaps;

        let tx_attributes_meet_cex_dex_criteria = !info.is_classified
            && info.is_private
            && (info
                .is_searcher_of_type_with_count_threshold(MevType::CexDexTrades, FILTER_THRESHOLD)
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
            possible_cex_dex.into_bundle(info, metadata)
        } else {
            self.utils.get_metrics().inspect(|m| {
                m.branch_filtering_trigger(MevType::CexDexTrades, "filter_possible_cex_dex")
            });
            None
        }
    }

    /// Filters out triangular arbitrage
    //TODO: Check for bug on tx:
    // https://dashboard.tenderly.co/tx/mainnet/0x310430b40132df960020af330b2e3b6a281751d45786f6b790e1cf1daf9a78bb?trace=0
    pub fn is_triangular_arb(&self, dex_swaps: &[NormalizedSwap]) -> bool {
        // Not enough swaps to form a cycle, thus cannot be arbitrage.
        if dex_swaps.len() < 2 {
            return false
        }

        let original_token = dex_swaps[0].token_in.address;
        let final_token = dex_swaps.last().unwrap().token_out.address;

        original_token == final_token
    }
}

pub fn max_arb_delta(tx_info: &TxInfo, pnl: &Rational) -> Rational {
    let mut base_diff = 3;

    if tx_info.is_labelled_searcher_of_type(MevType::CexDexQuotes)
        || tx_info.is_labelled_searcher_of_type(MevType::CexDexTrades)
    {
        if pnl < &Rational::from(5) {
            base_diff += 7;
        } else if pnl < &Rational::from(40) {
            base_diff += 5;
        } else if pnl < &Rational::from(100) {
            base_diff += 2;
        }
    } else if tx_info
        .contract_type
        .as_ref()
        .map_or(false, |c| c.is_mev_contract())
    {
        base_diff += 2;
    }

    Rational::from(base_diff)
}

#[cfg(test)]
mod tests {

    use alloy_primitives::hex;
    use brontes_types::constants::USDT_ADDRESS;

    use crate::{
        test_utils::{InspectorTestUtils, InspectorTxRunConfig},
        Inspectors,
    };

    //TODO: Verify that the load config is working as expected for both tests and
    // regular runs
    /*#[brontes_macros::test]
    async fn test_cex_dex_markout() {
        // https://etherscan.io/tx/0x6c9f2b9200d1f27501ad8bfc98fda659033e6242d3fd75f3f9c18e7fbc681ec2
        let inspector_util = InspectorTestUtils::new(USDT_ADDRESS, 40.5).await;

        let tx = hex!("6c9f2b9200d1f27501ad8bfc98fda659033e6242d3fd75f3f9c18e7fbc681ec2").into();

        let config = InspectorTxRunConfig::new(Inspectors::CexDexMarkout)
            .with_mev_tx_hashes(vec![tx])
            .with_gas_paid_usd(38.31)
            .with_expected_profit_usd(94.82)
            .with_block_time_weights_for_cex_pricing();

        inspector_util.run_inspector(config, None).await.unwrap();
    }*/

    #[brontes_macros::test]
    async fn test_cex_dex_markout_perl() {
        let inspector_util = InspectorTestUtils::new(USDT_ADDRESS, 0.5).await;
        // we have no trades in the timewindow
        let tx = hex!("b2684e6f02082288c34149d9564a1dc9d78ae901ab3e20194a1a873ebfe3d9ac").into();
        let config =
            InspectorTxRunConfig::new(Inspectors::CexDexMarkout).with_mev_tx_hashes(vec![tx]);

        inspector_util.assert_no_mev(config).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_cex_dex_markout_curve() {
        // missing trade
        let inspector_util = InspectorTestUtils::new(USDT_ADDRESS, 0.5).await;
        let tx = hex!("382b2ae940b7665b4b403bdd87f03dabfcc05bbe35ae82931ada06a8d60bb79a").into();
        let config =
            InspectorTxRunConfig::new(Inspectors::CexDexMarkout).with_mev_tx_hashes(vec![tx]);

        inspector_util.assert_no_mev(config).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_cex_dex_markout_eth_dai() {
        // no trades in db
        let inspector_util = InspectorTestUtils::new(USDT_ADDRESS, 0.5).await;
        let tx = hex!("60cbfc1b8b72479259c236e0ef17ffeade286f7c7821a03f6c180340b694f9c7").into();
        let config =
            InspectorTxRunConfig::new(Inspectors::CexDexMarkout).with_mev_tx_hashes(vec![tx]);

        inspector_util.assert_no_mev(config).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_cex_dex_markout_pepe_usdc() {
        let inspector_util = InspectorTestUtils::new(USDT_ADDRESS, 15.5).await;
        let tx = hex!("516cb79ee183619bf2f1542e847b84578fd8ca8ee926af1bdc3331fd73715ca3").into();
        let config = InspectorTxRunConfig::new(Inspectors::CexDexMarkout)
            .with_mev_tx_hashes(vec![tx])
            .with_expected_profit_usd(3.88)
            .with_gas_paid_usd(6.93);
        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_cex_dex_markout_bad_price() {
        // should be there if intermediary. however thats failing
        let inspector_util = InspectorTestUtils::new(USDT_ADDRESS, 15.5).await;
        let tx = hex!("5ce797b5b3f58a99f170ee7a4ac1fc1ca37600ad92944730c19f13ef05f568c7").into();
        let config = InspectorTxRunConfig::new(Inspectors::CexDexMarkout)
            .with_mev_tx_hashes(vec![tx])
            .with_expected_profit_usd(15.25)
            .with_gas_paid_usd(2.43);

        inspector_util.run_inspector(config, None).await.unwrap();
    }
}
