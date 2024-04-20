use std::{
    cmp::{max, min},
    fmt,
    fmt::Display,
    sync::Arc,
};

use brontes_database::libmdbx::LibmdbxReader;
use brontes_types::{
    db::{
        cex::{
            time_window_vwam::MakerTakerWindowVwam, vwam::MakerTaker, CexExchange, FeeAdjustedQuote,
        },
        dex::PriceAt,
    },
    mev::{ArbDetails, ArbPnl, Bundle, BundleData, CexDex, MevType},
    normalized_actions::{accounting::ActionAccounting, Actions, NormalizedSwap},
    pair::Pair,
    tree::{BlockTree, GasDetails},
    ActionIter, FastHashMap, ToFloatNearest, TreeSearchBuilder, TxInfo,
};
use colored::Colorize;
use itertools::Itertools;
use malachite::{
    num::basic::traits::{One, Two, Zero},
    Rational,
};
use reth_primitives::Address;
use tracing::error;

use crate::atomic_arb::is_stable_pair;

// The threshold for the number of CEX-DEX trades an address is required to make
// to classify a a negative pnl cex-dex trade as a CEX-DEX trade
pub const FILTER_THRESHOLD: u64 = 20;

use crate::{shared_utils::SharedInspectorUtils, Inspector, Metadata};

pub struct CexDexMarkoutInspector<'db, DB: LibmdbxReader> {
    utils:         SharedInspectorUtils<'db, DB>,
    cex_exchanges: Vec<CexExchange>,
}

impl<'db, DB: LibmdbxReader> CexDexMarkoutInspector<'db, DB> {
    pub fn new(quote: Address, db: &'db DB, cex_exchanges: &[CexExchange]) -> Self {
        Self {
            utils:         SharedInspectorUtils::new(quote, db),
            cex_exchanges: cex_exchanges.to_owned(),
        }
    }
}

impl<DB: LibmdbxReader> Inspector for CexDexMarkoutInspector<'_, DB> {
    type Result = Vec<Bundle>;

    fn get_id(&self) -> &str {
        "CexDexMarkout"
    }

    fn process_tree(&self, tree: Arc<BlockTree<Actions>>, metadata: Arc<Metadata>) -> Self::Result {
        if metadata.cex_trades.is_none() {
            tracing::warn!("no cex trades for block");
            return vec![]
        }

        tree.clone()
            .collect_all(TreeSearchBuilder::default().with_actions([
                Actions::is_swap,
                Actions::is_transfer,
                Actions::is_eth_transfer,
                Actions::is_aggregator,
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
                    .collect_action_vec(Actions::try_swaps_merged);

                if self.is_triangular_arb(&dex_swaps) {
                    return None
                }

                let mut possible_cex_dex: CexDexProcessing =
                    self.detect_cex_dex(dex_swaps, &metadata)?;

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

impl<DB: LibmdbxReader> CexDexMarkoutInspector<'_, DB> {
    pub fn detect_cex_dex(
        &self,
        dex_swaps: Vec<NormalizedSwap>,
        metadata: &Metadata,
    ) -> Option<CexDexProcessing> {
        let pricing = self.cex_trades_for_swap(&dex_swaps, metadata);

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
        let calcuation_vwam = (pricing_window_vwam.iter().flatten().count()
            >= pricing_vwam.iter().flatten().count())
        .then_some(pricing_window_vwam)
        .or(Some(pricing_vwam))
        .unwrap();

        let vwam_result = PossibleCexDex::from_exchange_legs(
            dex_swaps
                .iter()
                .zip(calcuation_vwam)
                .filter_map(|(swap, possible_pricing)| {
                    Some(self.profit_classifier(
                        swap,
                        possible_pricing?,
                        metadata,
                        CexExchange::VWAP,
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
                        .into_iter()
                        .map(|(ex, (m_price, _))| {
                            (
                                ex,
                                self.profit_classifier(
                                    &dex_swaps[i],
                                    (
                                        m_price.clone(),
                                        taker.get(&ex).map(|p| p.0.clone()).unwrap().clone(),
                                    ),
                                    metadata,
                                    *ex,
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
            .map(|swaps| PossibleCexDex::from_exchange_legs(swaps))
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

        let token_price = metadata
            .cex_trades
            .as_ref()
            .unwrap()
            .lock()
            .calculate_time_window_vwam(
                &self.cex_exchanges,
                Pair(swap.token_in.address, self.utils.quote),
                &vol,
                metadata.block_timestamp * 1000000,
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
    ) -> Vec<(Option<MakerTakerWindowVwam>, Option<MakerTaker>)> {
        dex_swaps
            .iter()
            .map(|swap| {
                let pair = Pair(swap.token_out.address, swap.token_in.address);

                metadata
                    .cex_trades
                    .as_ref()
                    .unwrap()
                    .lock()
                    .calculate_all_methods(
                        &self.cex_exchanges,
                        pair,
                        &swap.amount_out,
                        metadata.block_timestamp * 1000000,
                        None,
                    )
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
        tracing::info!(?gas_cost);

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

        println!("{}", cex_dex);
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

        if !sanity_check_arb.profitable_exchanges.is_empty() {
            return possible_cex_dex.into_bundle(info)
        }

        let has_outlier_pnl = sanity_check_arb.profitable_exchanges.len() < 2
            && !sanity_check_arb.profitable_exchanges.is_empty()
            && sanity_check_arb.profitable_exchanges[0].1.maker_taker_ask.1 > 10000
            && (sanity_check_arb.profitable_exchanges[0].0 == CexExchange::Kucoin
                || sanity_check_arb.profitable_exchanges[0].0 == CexExchange::Okex);

        let has_positive_exchange_pnl = !sanity_check_arb.profitable_exchanges.is_empty();

        if has_positive_exchange_pnl && !has_outlier_pnl
            || (!info.is_classified
                && (info.gas_details.coinbase_transfer.is_some()
                    && info.is_private
                    && info.is_searcher_of_type_with_count_threshold(
                        MevType::CexDex,
                        FILTER_THRESHOLD,
                    )
                    || info.is_cex_dex_call))
            || info.is_searcher_of_type_with_count_threshold(MevType::CexDex, FILTER_THRESHOLD * 5)
                && sanity_check_arb.profitable_cross_exchange
                && !sanity_check_arb.is_stable_swaps
            || info.is_searcher_of_type_with_count_threshold(MevType::CexDex, FILTER_THRESHOLD * 3)
                && has_outlier_pnl
                && !sanity_check_arb.is_stable_swaps
            || info.is_labelled_searcher_of_type(MevType::CexDex)
            || sanity_check_arb.global_profitability
                && sanity_check_arb.profitable_exchanges.len() > 3
        {
            possible_cex_dex.into_bundle(info)
        } else {
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
            self.max_profit
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
        let profitable_exchanges: Vec<(CexExchange, ArbPnl)> = self
            .per_exchange_pnl
            .iter()
            .filter_map(|p| p.as_ref())
            .filter(|p| {
                p.aggregate_pnl.maker_taker_mid.1 > Rational::ZERO
                    || p.aggregate_pnl.maker_taker_ask.1 > Rational::ZERO
            })
            .map(|p| (p.arb_legs[0].as_ref().unwrap().cex_quote.exchange, p.aggregate_pnl.clone()))
            .collect();

        let profitable_cross_exchange = self
            .max_profit
            .as_ref()
            .unwrap()
            .aggregate_pnl
            .maker_taker_mid
            .0
            > Rational::ZERO
            || self
                .max_profit
                .as_ref()
                .unwrap()
                .aggregate_pnl
                .maker_taker_ask
                .0
                > Rational::ZERO;

        let global_profitability = self.global_vmam_cex_dex.as_ref().map_or(false, |global| {
            global.aggregate_pnl.maker_taker_mid.0 > Rational::ZERO
                || global.aggregate_pnl.maker_taker_ask.0 > Rational::ZERO
        });

        let is_stable_swaps = self.is_stable_swaps();

        ArbSanityCheck {
            profitable_exchanges,
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

        for leg in exchange_legs.iter_mut() {
            if let Some(leg) = leg {
                total_mid_maker += &leg.pnl.maker_taker_mid.0;
                total_mid_taker += &leg.pnl.maker_taker_mid.1;
                total_ask_maker += &leg.pnl.maker_taker_ask.0;
                total_ask_taker += &leg.pnl.maker_taker_ask.1;
            }
        }

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
    pub profitable_exchanges:      Vec<(CexExchange, ArbPnl)>,
    pub profitable_cross_exchange: bool,
    pub global_profitability:      bool,
    pub is_stable_swaps:           bool,
}

impl fmt::Display for ArbSanityCheck {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "\x1b[1m\x1b[4mCex Dex Sanity Check\x1b[0m\x1b[24m")?;

        writeln!(f, "Profitable Exchanges:")?;
        for (index, (exchange, pnl)) in self.profitable_exchanges.iter().enumerate() {
            writeln!(f, "    - Exchange {}: {}", index + 1, exchange)?;
            writeln!(f, "        - ARB PNL: {}", pnl)?;
        }

        if self.profitable_cross_exchange {
            writeln!(f, "Is profitable cross exchange")?;
        } else {
            writeln!(f, "Is not profitable cross exchange")?;
        }

        if self.global_profitability {
            writeln!(f, "Is globally profitable based on cross exchange VMAP")?;
        } else {
            writeln!(f, "Is not globally profitable based on cross exchange VMAP")?;
        }

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
    error!(
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
    async fn test_cex_dex_markout_vs_non() {
        let inspector_util = InspectorTestUtils::new(USDT_ADDRESS, 0.5).await;

        let tx = hex!("21b129d221a4f169de0fc391fe0382dbde797b69300a9a68143487c54d620295").into();

        let config = InspectorTxRunConfig::new(Inspectors::CexDexMarkout)
            .with_mev_tx_hashes(vec![tx])
            .with_dex_prices()
            .needs_token(WETH_ADDRESS)
            .with_expected_profit_usd(123_317.44)
            .with_gas_paid_usd(80_751.62);

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
