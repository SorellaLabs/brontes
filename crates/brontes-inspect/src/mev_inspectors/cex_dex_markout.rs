use std::{
    cmp::{max, min},
    fmt,
    fmt::Display,
    sync::Arc,
};

use alloy_primitives::FixedBytes;
use brontes_database::libmdbx::LibmdbxReader;
use brontes_metrics::inspectors::OutlierMetrics;
use brontes_types::{
    db::{
        cex::{
            time_window_vwam::MakerTakerWindowVWAP, vwam::MakerTaker, CexExchange, FeeAdjustedQuote,
        },
        dex::PriceAt,
    },
    display::utils::format_etherscan_url,
    mev::{ArbDetails, ArbPnl, Bundle, BundleData, CexDex, MevType},
    normalized_actions::{accounting::ActionAccounting, Action, NormalizedSwap},
    pair::Pair,
    tree::{BlockTree, GasDetails},
    ActionIter, FastHashMap, ToFloatNearest, TreeSearchBuilder, TxInfo,
};
use colored::Colorize;
use itertools::{multizip, Itertools};
use malachite::{
    num::basic::traits::{One, Two, Zero},
    Rational,
};
use reth_primitives::Address;
use tracing::{trace, warn};

use crate::atomic_arb::is_stable_pair;

// The threshold for the number of CEX-DEX trades an address is required to make
// to classify a a negative pnl cex-dex trade as a CEX-DEX trade
pub const FILTER_THRESHOLD: u64 = 20;
pub const HIGH_PROFIT_THRESHOLD: Rational = Rational::const_from_unsigned(10000);

use crate::{shared_utils::SharedInspectorUtils, Inspector, Metadata};

pub struct CexDexMarkoutInspector<'db, DB: LibmdbxReader> {
    utils:         SharedInspectorUtils<'db, DB>,
    cex_exchanges: Vec<CexExchange>,
}

impl<'db, DB: LibmdbxReader> CexDexMarkoutInspector<'db, DB> {
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

impl<DB: LibmdbxReader> Inspector for CexDexMarkoutInspector<'_, DB> {
    type Result = Vec<Bundle>;

    fn get_id(&self) -> &str {
        "CexDexMarkout"
    }

    fn get_quote_token(&self) -> Address {
        self.utils.quote
    }

    fn process_tree(&self, tree: Arc<BlockTree<Action>>, metadata: Arc<Metadata>) -> Self::Result {
        if metadata.cex_trades.is_none() {
            tracing::warn!("no cex trades for block");
            return vec![]
        }

        self.utils
            .get_metrics()
            .map(|m| {
                m.run_inspector(MevType::CexDex, || {
                    self.process_tree_inner(tree.clone(), metadata.clone())
                })
            })
            .unwrap_or_else(|| self.process_tree_inner(tree.clone(), metadata.clone()))
    }
}

impl<DB: LibmdbxReader> CexDexMarkoutInspector<'_, DB> {
    fn process_tree_inner(
        &self,
        tree: Arc<BlockTree<Action>>,
        metadata: Arc<Metadata>,
    ) -> Vec<Bundle> {
        let (hashes, swaps): (Vec<_>, Vec<_>) = tree
            .clone()
            .collect_all(TreeSearchBuilder::default().with_actions([
                Action::is_swap,
                Action::is_transfer,
                Action::is_eth_transfer,
                Action::is_aggregator,
            ]))
            .unzip();

        let tx_info = tree.get_tx_info_batch(&hashes, self.utils.db);
        multizip((swaps, tx_info))
            .filter_map(|(swaps, tx_info)| {
                let tx_info = tx_info?;

                // Return early if the tx is a solver settling trades
                if let Some(contract_type) = tx_info.contract_type.as_ref() {
                    if contract_type.is_solver_settlement() || contract_type.is_defi_automation() {
                        trace!(
                            target: "brontes::cex-dex-markout",
                            "Filtered out CexDex tx because it is a contract of type {:?}\n Tx: {}",
                            contract_type,
                            format_etherscan_url(&tx_info.tx_hash)
                        );
                        self.utils.get_metrics().inspect(|m| {
                            m.branch_filtering_trigger(
                                MevType::CexDex,
                                "is_solver_settlement_or_defi_automation",
                            )
                        });
                        return None
                    }
                }

                let deltas = swaps.clone().into_iter().account_for_actions();
                let dex_swaps = self
                    .utils
                    .flatten_nested_actions(swaps.into_iter(), &|action| action.is_swap())
                    .collect_action_vec(Action::try_swaps_merged);

                if self.is_triangular_arb(&dex_swaps) {
                    trace!(
                        target: "brontes::cex-dex-markout",
                        "Filtered out CexDex because it is a triangular arb\n Tx: {}",
                        format_etherscan_url(&tx_info.tx_hash)
                    );
                    self.utils.get_metrics().inspect(|m| {
                        m.branch_filtering_trigger(MevType::CexDex, "is_triangular_arb")
                    });

                    return None
                }

                let mut possible_cex_dex: CexDexProcessing = self.detect_cex_dex(
                    dex_swaps,
                    &metadata,
                    tx_info.is_searcher_of_type(MevType::CexDex)
                        || tx_info.is_labelled_searcher_of_type(MevType::CexDex),
                    tx_info.tx_hash,
                )?;

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

    pub fn detect_cex_dex(
        &self,
        dex_swaps: Vec<NormalizedSwap>,
        metadata: &Metadata,
        marked_cex_dex: bool,
        tx_hash: FixedBytes<32>,
    ) -> Option<CexDexProcessing> {
        let pricing = self.cex_trades_for_swap(&dex_swaps, metadata, marked_cex_dex, tx_hash);

        // pricing window
        let pricing_window_vwam = pricing
            .iter()
            .map(|trade| {
                let some_pricings = trade.0.as_ref()?;
                Some((
                    some_pricings.0.global_exchange_price.clone(),
                    some_pricings.1.global_exchange_price.clone(),
                ))
            })
            .collect_vec();

        // optimistic volume clearance
        let pricing_vwam = pricing
            .iter()
            .map(|trade| {
                let some_pricings = trade.1.as_ref()?;
                Some((some_pricings.0.price.clone(), some_pricings.1.price.clone()))
            })
            .collect_vec();

        // per exchange volumes
        let pricing_window_vwam_per_ex = pricing
            .iter()
            .map(|trade| {
                let some_pricings = trade.0.as_ref()?;
                Some((
                    some_pricings.0.exchange_price_with_volume_direct.clone(),
                    some_pricings.1.exchange_price_with_volume_direct.clone(),
                ))
            })
            .collect_vec();

        // use the setup that has more hops. if they are equal,
        // use the new version.
        let calculation_vwam = if pricing_window_vwam.iter().flatten().count()
            >= pricing_vwam.iter().flatten().count()
        {
            pricing_window_vwam
        } else {
            pricing_vwam
        };

        let vwam_result = PossibleCexDex::from_exchange_legs(
            dex_swaps
                .iter()
                .zip(calculation_vwam)
                .filter_map(|(swap, possible_pricing)| {
                    Some(self.profit_classifier(
                        swap,
                        possible_pricing?,
                        metadata,
                        CexExchange::VWAP,
                        tx_hash,
                    ))
                })
                .collect_vec(),
        );

        let per_exchange_pnl = pricing_window_vwam_per_ex
            .iter()
            .enumerate()
            .filter_map(|(i, stuff)| {
                let (maker, taker) = stuff.as_ref()?;

                Some(
                    maker
                        .iter()
                        .map(|(ex, (m_price, _))| {
                            (
                                ex,
                                self.profit_classifier(
                                    &dex_swaps[i],
                                    (
                                        m_price.clone(),
                                        taker.get(ex).map(|p| p.0.clone()).unwrap().clone(),
                                    ),
                                    metadata,
                                    *ex,
                                    tx_hash,
                                ),
                            )
                        })
                        .collect_vec(),
                )
            })
            .fold(
                FastHashMap::default(),
                |mut acc: FastHashMap<CexExchange, Vec<Option<ExchangeLeg>>>, x| {
                    for (ex, data) in x {
                        acc.entry(*ex).or_default().push(data);
                    }
                    acc
                },
            )
            .into_values()
            .map(PossibleCexDex::from_exchange_legs)
            .collect_vec();

        CexDexProcessing::new(dex_swaps, vwam_result, per_exchange_pnl)
    }

    /// For a given swap & CEX quote, calculates the potential profit from
    /// buying on DEX and selling on CEX.
    fn profit_classifier(
        &self,
        swap: &NormalizedSwap,
        cex_quote: (Rational, Rational),
        metadata: &Metadata,
        exchange: CexExchange,
        tx_hash: FixedBytes<32>,
    ) -> Option<ExchangeLeg> {
        // If the price difference between the DEX and CEX is greater than 2x, the
        // quote is likely invalid

        let swap_rate = swap.swap_rate();
        let smaller = min(&swap_rate, &cex_quote.0);
        let larger = max(&swap_rate, &cex_quote.0);

        if smaller * Rational::TWO < *larger {
            log_price_delta(
                swap.token_in_symbol(),
                swap.token_out_symbol(),
                swap.swap_rate().clone().to_float(),
                cex_quote.0.clone().to_float(),
                &swap.token_in.address,
                &swap.token_out.address,
            );

            return None
        }

        // A positive delta indicates potential profit from buying on DEX
        // and selling on CEX.
        let maker_delta = &cex_quote.0 - swap.swap_rate();
        let taker_delta = &cex_quote.1 - swap.swap_rate();

        let vol = Rational::ONE;

        let pair = Pair(self.utils.quote, swap.token_in.address);
        let token_price = metadata
            .cex_trades
            .as_ref()
            .unwrap()
            .lock()
            .calculate_time_window_vwam(
                &self.cex_exchanges,
                pair,
                &vol,
                metadata.block_timestamp * 1000000,
                true,
                swap,
                tx_hash,
            )?
            .0
            .global_exchange_price;

        let pnl_mid = (
            &maker_delta * &swap.amount_out * &token_price,
            &taker_delta * &swap.amount_out * &token_price,
        );

        let quote = FeeAdjustedQuote {
            timestamp: metadata.block_timestamp,
            price_maker: (cex_quote.0.clone(), cex_quote.0.clone()),
            price_taker: (cex_quote.1.clone(), cex_quote.1.clone()),
            amount: (Rational::ONE, Rational::ONE),
            token0: Address::ZERO,
            exchange,
        };

        Some(ExchangeLeg {
            cex_quote: quote,
            pnl:       ArbPnl { maker_taker_mid: pnl_mid.clone(), maker_taker_ask: pnl_mid },
        })
    }

    /// Retrieves CEX quotes for a DEX swap, analyzing both direct and
    /// intermediary token pathways.
    fn cex_trades_for_swap(
        &self,
        dex_swaps: &[NormalizedSwap],
        metadata: &Metadata,
        marked_cex_dex: bool,
        tx_hash: FixedBytes<32>,
    ) -> Vec<(Option<MakerTakerWindowVWAP>, Option<MakerTaker>)> {
        dex_swaps
            .iter()
            .filter(|swap| swap.amount_out != Rational::ZERO)
            .map(|swap| {
                let pair = Pair(swap.token_in.address, swap.token_out.address);

                let window = self
                    .utils
                    .get_metrics()
                    .map(|m| {
                        m.run_cex_price_window(|| {
                            metadata
                                .cex_trades
                                .as_ref()
                                .unwrap()
                                .lock()
                                .calculate_time_window_vwam(
                                    &self.cex_exchanges,
                                    pair,
                                    &swap.amount_out,
                                    metadata.microseconds_block_timestamp(),
                                    marked_cex_dex,
                                    swap,
                                    tx_hash,
                                )
                        })
                    })
                    .unwrap_or_else(|| {
                        metadata
                            .cex_trades
                            .as_ref()
                            .unwrap()
                            .lock()
                            .calculate_time_window_vwam(
                                &self.cex_exchanges,
                                pair,
                                &swap.amount_out,
                                metadata.microseconds_block_timestamp(),
                                marked_cex_dex,
                                swap,
                                tx_hash,
                            )
                    });

                let other = self
                    .utils
                    .get_metrics()
                    .map(|m| {
                        m.run_cex_price_vol(|| {
                            metadata
                                .cex_trades
                                .as_ref()
                                .unwrap()
                                .lock()
                                .get_optimistic_vmap(
                                    &self.cex_exchanges,
                                    &pair,
                                    &swap.amount_out,
                                    None,
                                    marked_cex_dex,
                                    swap,
                                    tx_hash,
                                )
                        })
                    })
                    .unwrap_or_else(|| {
                        metadata
                            .cex_trades
                            .as_ref()
                            .unwrap()
                            .lock()
                            .get_optimistic_vmap(
                                &self.cex_exchanges,
                                &pair,
                                &swap.amount_out,
                                None,
                                marked_cex_dex,
                                swap,
                                tx_hash,
                            )
                    });

                if (window.is_none() || other.is_none()) && marked_cex_dex {
                    self.utils
                        .get_metrics()
                        .inspect(|m| m.missing_cex_pair(pair));
                }

                (window, other)
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
            self.utils.get_metrics().inspect(|m| {
                m.branch_filtering_trigger(MevType::CexDex, "filter_possible_cex_dex")
            });
            None
        }
    }

    /// Filters out triangular arbitrage
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

#[derive(Debug)]
pub struct CexDexProcessing {
    pub dex_swaps:           Vec<NormalizedSwap>,
    pub global_vmam_cex_dex: Option<PossibleCexDex>,
    pub per_exchange_pnl:    Vec<Option<PossibleCexDex>>,
    pub max_profit:          Option<PossibleCexDex>,
}

impl CexDexProcessing {
    pub fn new(
        dex_swaps: Vec<NormalizedSwap>,
        global_vmam_cex_dex: Option<PossibleCexDex>,
        per_exchange_pnl: Vec<Option<PossibleCexDex>>,
    ) -> Option<Self> {
        let mut this = Self { per_exchange_pnl, dex_swaps, max_profit: None, global_vmam_cex_dex };
        this.construct_max_profit_route()?;
        Some(this)
    }

    pub fn construct_max_profit_route(&mut self) -> Option<()> {
        if self.per_exchange_pnl.iter().all(Option::is_none) {
            return None
        }

        let mut transposed_arb_leg: Vec<Vec<&ExchangeLeg>> = vec![Vec::new(); self.dex_swaps.len()];
        let mut incomplete_routes: Vec<usize> = Vec::new();

        for (index, p) in self
            .per_exchange_pnl
            .iter()
            .enumerate()
            .filter_map(|(i, opt)| opt.as_ref().map(|p| (i, p)))
        {
            let mut is_complete = true;
            for (i, arb_leg) in p.arb_legs.iter().enumerate() {
                if let Some(arb) = arb_leg {
                    transposed_arb_leg[i].push(arb);
                } else {
                    is_complete = false;
                }
            }

            if !is_complete {
                incomplete_routes.push(index);
            }
        }

        let best_pnls: Vec<Option<ExchangeLeg>> = transposed_arb_leg
            .into_iter()
            .map(|arb_legs| {
                arb_legs
                    .into_iter()
                    .max_by_key(|arb_leg| arb_leg.pnl.clone())
                    .cloned()
            })
            .collect();

        let aggregate_pnl = best_pnls
            .iter()
            .filter_map(|p| p.as_ref())
            .map(|x| x.pnl.clone())
            .reduce(|acc, x| acc + x)
            .unwrap_or_default();

        self.max_profit = Some(PossibleCexDex { arb_legs: best_pnls, aggregate_pnl });

        incomplete_routes.iter().rev().for_each(|i| {
            self.per_exchange_pnl.remove(*i);
        });

        Some(())
    }

    pub fn adjust_for_gas_cost(&mut self, gas_cost: &Rational) {
        self.per_exchange_pnl.iter_mut().for_each(|exchange_arb| {
            if let Some(arb) = exchange_arb {
                arb.adjust_for_gas_cost(gas_cost);
            }
        });

        if let Some(arb) = self.max_profit.as_mut() {
            arb.adjust_for_gas_cost(gas_cost)
        }

        if let Some(arb) = self.global_vmam_cex_dex.as_mut() {
            arb.adjust_for_gas_cost(gas_cost)
        }
    }

    pub fn into_bundle(self, tx_info: &TxInfo) -> Option<(f64, BundleData)> {
        Some((
            self.global_vmam_cex_dex
                .as_ref()?
                .aggregate_pnl
                .maker_taker_mid
                .0
                .clone()
                .to_float(),
            BundleData::CexDex(CexDex {
                tx_hash:             tx_info.tx_hash,
                global_vmap_pnl:     self.global_vmam_cex_dex.as_ref()?.aggregate_pnl.clone(),
                global_vmap_details: self
                    .global_vmam_cex_dex?
                    .generate_arb_details(&self.dex_swaps),

                optimal_route_details: self
                    .max_profit
                    .as_ref()?
                    .generate_arb_details(&self.dex_swaps),
                optimal_route_pnl:     self.max_profit.as_ref().unwrap().aggregate_pnl.clone(),
                per_exchange_pnl:      self
                    .per_exchange_pnl
                    .iter()
                    .map(|p| p.as_ref().unwrap())
                    .map(|p| {
                        let leg = p.arb_legs.first().unwrap();
                        (leg.clone(), p.aggregate_pnl.clone())
                    })
                    .map(|(leg, pnl)| (leg.unwrap().cex_quote.exchange, pnl))
                    .collect(),

                per_exchange_details: self
                    .per_exchange_pnl
                    .iter()
                    .filter_map(|p| p.as_ref().map(|p| p.generate_arb_details(&self.dex_swaps)))
                    .collect(),

                gas_details: tx_info.gas_details,
                swaps:       self.dex_swaps,
            }),
        ))
    }

    fn arb_sanity_check(&self) -> ArbSanityCheck {
        let (profitable_exchanges_mid, profitable_exchanges_ask) = self
            .per_exchange_pnl
            .iter()
            .filter_map(|p| p.as_ref())
            .fold((Vec::new(), Vec::new()), |(mut mid, mut ask), p| {
                if p.aggregate_pnl.maker_taker_mid.0 > Rational::ZERO {
                    mid.push((
                        p.arb_legs[0].as_ref().unwrap().cex_quote.exchange,
                        p.aggregate_pnl.clone(),
                    ));
                }
                if p.aggregate_pnl.maker_taker_ask.0 > Rational::ZERO {
                    ask.push((
                        p.arb_legs[0].as_ref().unwrap().cex_quote.exchange,
                        p.aggregate_pnl.clone(),
                    ));
                }
                (mid, ask)
            });

        let profitable_cross_exchange = {
            let mid_price_profitability = self
                .max_profit
                .as_ref()
                .expect(
                    "Max profit should always exist, CexDex inspector should have returned early",
                )
                .aggregate_pnl
                .maker_taker_mid
                .0
                > Rational::ZERO;

            let ask_price_profitability = self
                .max_profit
                .as_ref()
                .unwrap()
                .aggregate_pnl
                .maker_taker_ask
                .0
                > Rational::ZERO;

            (mid_price_profitability, ask_price_profitability)
        };

        let global_profitability =
            self.global_vmam_cex_dex
                .as_ref()
                .map_or((false, false), |global| {
                    (
                        global.aggregate_pnl.maker_taker_mid.0 > Rational::ZERO,
                        global.aggregate_pnl.maker_taker_ask.0 > Rational::ZERO,
                    )
                });

        let is_stable_swaps = self.is_stable_swaps();

        ArbSanityCheck {
            profitable_exchanges_mid,
            profitable_exchanges_ask,
            profitable_cross_exchange,
            global_profitability,
            is_stable_swaps,
        }
    }

    fn is_stable_swaps(&self) -> bool {
        self.dex_swaps
            .iter()
            .all(|swap| is_stable_pair(swap.token_in_symbol(), swap.token_out_symbol()))
    }
}

impl fmt::Display for CexDexProcessing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", "Cex-Dex Processing Details:".bold().underline())?;

        writeln!(f, "{}", "Dex Swaps:".bold())?;
        for swap in &self.dex_swaps {
            writeln!(f, "  - {}", swap)?;
        }

        writeln!(f, "{}", "Global VMAM CEX/DEX:".bold())?;
        if let Some(ref vmam) = self.global_vmam_cex_dex {
            writeln!(f, "  - {}", vmam)?;
        } else {
            writeln!(f, "  - Not available")?;
        }

        writeln!(f, "{}", "Per Exchange PnL:".bold())?;
        for (index, exchange_pnl) in self.per_exchange_pnl.iter().enumerate() {
            writeln!(
                f,
                "  - Exchange {}: {}",
                index + 1,
                exchange_pnl
                    .as_ref()
                    .map_or("PnL data not available".to_string(), |pnl| pnl.to_string())
            )?;
        }

        writeln!(f, "{}", "Max Profit:".bold())?;
        match self.max_profit {
            Some(ref max) => writeln!(f, "  - {}", max)?,
            None => writeln!(f, "  - Not available")?,
        }

        Ok(())
    }
}
#[derive(Debug, Default)]
pub struct PossibleCexDex {
    pub arb_legs:      Vec<Option<ExchangeLeg>>,
    pub aggregate_pnl: ArbPnl,
}

impl PossibleCexDex {
    pub fn from_exchange_legs(mut exchange_legs: Vec<Option<ExchangeLeg>>) -> Option<Self> {
        if exchange_legs.iter().all(Option::is_none) {
            return None
        }
        let mut total_mid_maker = Rational::ZERO;
        let mut total_mid_taker = Rational::ZERO;
        let mut total_ask_maker = Rational::ZERO;
        let mut total_ask_taker = Rational::ZERO;

        exchange_legs.iter_mut().flatten().for_each(|leg| {
            total_mid_maker += &leg.pnl.maker_taker_mid.0;
            total_mid_taker += &leg.pnl.maker_taker_mid.1;
            total_ask_maker += &leg.pnl.maker_taker_ask.0;
            total_ask_taker += &leg.pnl.maker_taker_ask.1;
        });

        let aggregate_pnl = ArbPnl {
            maker_taker_mid: (total_mid_maker, total_mid_taker),
            maker_taker_ask: (total_ask_maker, total_ask_taker),
        };

        Some(PossibleCexDex { arb_legs: exchange_legs, aggregate_pnl })
    }

    pub fn adjust_for_gas_cost(&mut self, gas_cost: &Rational) {
        let maker_taker_mid = (
            &self.aggregate_pnl.maker_taker_mid.0 - gas_cost,
            &self.aggregate_pnl.maker_taker_mid.1 - gas_cost,
        );

        let maker_taker_ask = (
            &self.aggregate_pnl.maker_taker_ask.0 - gas_cost,
            &self.aggregate_pnl.maker_taker_ask.1 - gas_cost,
        );

        self.aggregate_pnl = ArbPnl { maker_taker_mid, maker_taker_ask };
    }

    pub fn generate_arb_details(&self, normalized_swaps: &[NormalizedSwap]) -> Vec<ArbDetails> {
        self.arb_legs
            .iter()
            .enumerate()
            .filter_map(|(index, arb_leg)| {
                arb_leg.as_ref().and_then(|leg| {
                    normalized_swaps.get(index).map(|swap| ArbDetails {
                        cex_exchange:   leg.cex_quote.exchange,
                        best_bid_maker: leg.cex_quote.price_maker.0.clone(),
                        best_ask_maker: leg.cex_quote.price_maker.1.clone(),
                        best_bid_taker: leg.cex_quote.price_taker.0.clone(),
                        best_ask_taker: leg.cex_quote.price_taker.1.clone(),
                        dex_exchange:   swap.protocol,
                        dex_price:      swap.swap_rate(),
                        dex_amount:     swap.amount_out.clone(),
                        pnl_pre_gas:    leg.pnl.clone(),
                    })
                })
            })
            .collect::<Vec<_>>()
    }
}

impl fmt::Display for PossibleCexDex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", "Aggregate PnL:".bold().underline())?;
        writeln!(f, "  {}", self.aggregate_pnl)?;

        writeln!(f, "{}", "Arbitrage Legs:".bold().underline())?;
        if !self.arb_legs.is_empty() {
            for (index, leg) in self.arb_legs.iter().enumerate() {
                match leg {
                    Some(leg) => writeln!(f, "  - Leg {}: {}", index + 1, leg)?,
                    None => writeln!(f, "  - Leg {}: No data available", index + 1)?,
                }
            }
        } else {
            writeln!(f, "  No arbitrage legs data available")?;
        }

        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct ArbSanityCheck {
    pub profitable_exchanges_mid:  Vec<(CexExchange, ArbPnl)>,
    pub profitable_exchanges_ask:  Vec<(CexExchange, ArbPnl)>,
    pub profitable_cross_exchange: (bool, bool),
    pub global_profitability:      (bool, bool),
    pub is_stable_swaps:           bool,
}

impl ArbSanityCheck {
    /// Determines if the CEX-DEX arbitrage is a highly profitable outlier.
    ///
    /// This function checks if the arbitrage is only profitable on a single
    /// exchange based on the ask price, and if the profit on this exchange
    /// exceeds a high profit threshold (e.g., $10,000). Additionally, it
    /// verifies if the exchange is either Kucoin or Okex.
    ///
    /// Returns `true` if all conditions are met, indicating a highly profitable
    /// outlier.
    pub fn is_profitable_outlier(&self) -> bool {
        !self.profitable_exchanges_ask.is_empty()
            && self.profitable_exchanges_ask.len() == 1
            && self.profitable_exchanges_ask[0].1.maker_taker_ask.1 > HIGH_PROFIT_THRESHOLD
            && (self.profitable_exchanges_ask[0].0 == CexExchange::Kucoin
                || self.profitable_exchanges_ask[0].0 == CexExchange::Okex)
    }
}

impl fmt::Display for ArbSanityCheck {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "\x1b[1m\x1b[4mCex Dex Sanity Check\x1b[0m\x1b[24m")?;

        writeln!(f, "Profitable Exchanges Based on Mid Price:")?;
        for (index, (exchange, pnl)) in self.profitable_exchanges_mid.iter().enumerate() {
            writeln!(f, "    - Exchange {}: {}", index + 1, exchange)?;
            writeln!(f, "        - ARB PNL: {}", pnl)?;
        }

        writeln!(f, "Profitable Exchanges Based on Ask Price:")?;
        for (index, (exchange, pnl)) in self.profitable_exchanges_ask.iter().enumerate() {
            writeln!(f, "    - Exchange {}: {}", index + 1, exchange)?;
            writeln!(f, "        - ARB PNL: {}", pnl)?;
        }

        writeln!(
            f,
            "Is profitable cross exchange (Mid Price): {}",
            if self.profitable_cross_exchange.0 { "Yes" } else { "No" }
        )?;
        writeln!(
            f,
            "Is profitable cross exchange (Ask Price): {}",
            if self.profitable_cross_exchange.1 { "Yes" } else { "No" }
        )?;

        writeln!(
            f,
            "Is globally profitable based on cross exchange VMAP (Mid Price): {}",
            if self.global_profitability.0 { "Yes" } else { "No" }
        )?;
        writeln!(
            f,
            "Is globally profitable based on cross exchange VMAP (Ask Price): {}",
            if self.global_profitability.1 { "Yes" } else { "No" }
        )?;

        if self.is_stable_swaps {
            writeln!(f, "Is a stable swap")?;
        } else {
            writeln!(f, "Is not a stable swap")?;
        }

        Ok(())
    }
}

impl Display for ExchangeLeg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Cex Quote: {}, PnL: {}", self.cex_quote, self.pnl)
    }
}

#[derive(Clone, Debug)]
pub struct ExchangeLeg {
    pub cex_quote: FeeAdjustedQuote,
    pub pnl:       ArbPnl,
}

fn log_price_delta(
    token_in_symbol: &str,
    token_out_symbol: &str,
    dex_swap_rate: f64,
    cex_price: f64,
    token_in_address: &Address,
    token_out_address: &Address,
) {
    warn!(
        "\n\x1b[1;35mDetected significant price delta for direct pair for {} - {}:\x1b[0m\n\
         - \x1b[1;36mDEX Swap Rate:\x1b[0m {:.7}\n\
         - \x1b[1;36mCEX Price:\x1b[0m {:.7}\n\
         - Token Contracts:\n\
           * Token In: https://etherscan.io/address/{}\n\
           * Token Out: https://etherscan.io/address/{}",
        token_in_symbol,
        token_out_symbol,
        dex_swap_rate,
        cex_price,
        token_in_address,
        token_out_address
    );
}

#[cfg(test)]
mod tests {

    use alloy_primitives::hex;
    use brontes_types::constants::{USDT_ADDRESS, WETH_ADDRESS};

    use crate::{
        test_utils::{InspectorTestUtils, InspectorTxRunConfig},
        Inspectors,
    };

    #[brontes_macros::test]
    async fn test_cex_dex_markout() {
        // https://etherscan.io/tx/0x6c9f2b9200d1f27501ad8bfc98fda659033e6242d3fd75f3f9c18e7fbc681ec2
        let inspector_util = InspectorTestUtils::new(USDT_ADDRESS, 0.5).await;

        let tx = hex!("6c9f2b9200d1f27501ad8bfc98fda659033e6242d3fd75f3f9c18e7fbc681ec2").into();

        let config = InspectorTxRunConfig::new(Inspectors::CexDexMarkout)
            .with_mev_tx_hashes(vec![tx])
            .with_dex_prices()
            .needs_token(WETH_ADDRESS)
            .with_gas_paid_usd(38.31)
            .with_expected_profit_usd(148.430);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn text_curve_cex_dex() {
        // https://etherscan.io/tx/0x6c9f2b9200d1f27501ad8bfc98fda659033e6242d3fd75f3f9c18e7fbc681ec2
        let inspector_util = InspectorTestUtils::new(USDT_ADDRESS, 0.5).await;

        let tx = hex!("eb1e83b44f713de3acc7b056cbb233065420e73972a6e8bb3ec0000a88c9521f").into();

        let config = InspectorTxRunConfig::new(Inspectors::CexDexMarkout)
            .with_mev_tx_hashes(vec![tx])
            .with_dex_prices()
            .needs_token(WETH_ADDRESS)
            .with_gas_paid_usd(16.34)
            .with_expected_profit_usd(148.430);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_cex_dex_markout_vs_non() {
        let inspector_util = InspectorTestUtils::new(USDT_ADDRESS, 0.5).await;

        let tx = hex!("21b129d221a4f169de0fc391fe0382dbde797b69300a9a68143487c54d620295").into();

        let config = InspectorTxRunConfig::new(Inspectors::CexDexMarkout)
            .with_mev_tx_hashes(vec![tx])
            .with_expected_profit_usd(8958.161528605704)
            .with_gas_paid_usd(79748.18);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_cex_dex_markout_psm() {
        // https://etherscan.io/tx/0x5ea3ca12cac835172fa24066c6d895886c1917005e06d7b49b48cc99d5750557
        let inspector_util = InspectorTestUtils::new(USDT_ADDRESS, 0.5).await;

        let tx = hex!("5ea3ca12cac835172fa24066c6d895886c1917005e06d7b49b48cc99d5750557").into();

        let config = InspectorTxRunConfig::new(Inspectors::CexDexMarkout)
            .with_mev_tx_hashes(vec![tx])
            .with_dex_prices()
            .needs_token(WETH_ADDRESS)
            .with_expected_profit_usd(123_317.44)
            .with_gas_paid_usd(67.89);

        inspector_util.run_inspector(config, None).await.unwrap();
    }
}
