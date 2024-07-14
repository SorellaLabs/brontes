use std::{fmt, fmt::Display, sync::Arc};

use alloy_primitives::FixedBytes;
use brontes_types::{
    db::cex::{config::CexDexTradeConfig, CexExchange, FeeAdjustedQuote},
    mev::{ArbDetails, ArbPnl, BundleData, CexDex, OptimisticTrade},
    normalized_actions::NormalizedSwap,
    pair::Pair,
    ToFloatNearest, TxInfo,
};
use colored::Colorize;
use itertools::Itertools;
use malachite::{num::basic::traits::Zero, Rational};
use reth_primitives::Address;
use tracing::warn;

use crate::atomic_arb::is_stable_pair;

pub const HIGH_PROFIT_THRESHOLD: Rational = Rational::const_from_unsigned(10000);

use super::cex_dex_markout::PriceCalcType;
use crate::Metadata;

#[derive(Debug)]
pub struct CexDexProcessing {
    pub dex_swaps:           Vec<NormalizedSwap>,
    pub global_vmam_cex_dex: Option<PossibleCexDex>,
    pub per_exchange_pnl:    Vec<Option<PossibleCexDex>>,
    pub max_profit:          Option<PossibleCexDex>,
    pub optimstic_details:   Option<OptimisticDetails>,
}

impl CexDexProcessing {
    pub fn new(
        dex_swaps: Vec<NormalizedSwap>,
        global_vmam_cex_dex: Option<PossibleCexDex>,
        per_exchange_pnl: Vec<Option<PossibleCexDex>>,
        optimstic_details: Option<OptimisticDetails>,
    ) -> Option<Self> {
        let mut this = Self {
            per_exchange_pnl,
            dex_swaps,
            max_profit: None,
            global_vmam_cex_dex,
            optimstic_details,
        };
        this.construct_max_profit_route()?;
        Some(this)
    }

    pub fn construct_max_profit_route(&mut self) -> Option<()> {
        if self.per_exchange_pnl.iter().all(Option::is_none) {
            return None
        }

        let mut transposed_arb_leg: Vec<Vec<_>> = vec![Vec::new(); self.dex_swaps.len()];
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

        let best_pnls: Vec<Option<_>> = transposed_arb_leg
            .into_iter()
            .map(|arb_legs| {
                arb_legs
                    .into_iter()
                    .max_by_key(|arb_leg| arb_leg.0.pnl.clone())
                    .cloned()
            })
            .collect();

        let aggregate_pnl = best_pnls
            .iter()
            .filter_map(|p| p.as_ref())
            .map(|x| x.0.pnl.clone())
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

    pub fn into_bundle(
        self,
        tx_info: &TxInfo,
        config: &CexDexTradeConfig,
        meta: Arc<Metadata>,
    ) -> Option<(f64, BundleData, Vec<ExchangeLegCexPrice>)> {
        let optimistic = self
            .optimstic_details
            .as_ref()
            .map(|o| o.route_pnl().maker_taker_mid.0);
        let window = self
            .global_vmam_cex_dex
            .as_ref()?
            .aggregate_pnl
            .maker_taker_mid
            .0
            .clone();

        Some((
            optimistic.max(Some(window))?.to_float(),
            BundleData::CexDex(CexDex {
                block_number:        meta.block_num,
                tx_hash:             tx_info.tx_hash,
                global_vmap_pnl:     self.global_vmam_cex_dex.as_ref()?.aggregate_pnl.clone(),
                global_vmap_details: self
                    .global_vmam_cex_dex?
                    .generate_arb_details(&self.dex_swaps),

                optimal_route_details:    self
                    .max_profit
                    .as_ref()?
                    .generate_arb_details(&self.dex_swaps),
                optimal_route_pnl:        self.max_profit.as_ref().unwrap().aggregate_pnl.clone(),
                per_exchange_pnl:         self
                    .per_exchange_pnl
                    .iter()
                    .map(|p| p.as_ref().unwrap())
                    .map(|p| {
                        let leg = p.arb_legs.first().unwrap();
                        (leg.clone(), p.aggregate_pnl.clone())
                    })
                    .map(|(leg, pnl)| (leg.unwrap().0.cex_quote.exchange, pnl))
                    .collect(),
                optimistic_route_details: self
                    .optimstic_details
                    .as_ref()
                    .map(|r| r.optimistic_route_details.clone())
                    .unwrap_or_default(),
                optimistic_trade_details: self
                    .optimstic_details
                    .as_ref()
                    .map(|r| r.optimistic_trade_details.clone())
                    .unwrap_or_default(),
                optimistic_route_pnl:     self.optimstic_details.map(|o| o.route_pnl()),
                per_exchange_details:     self
                    .per_exchange_pnl
                    .iter()
                    .filter_map(|p| p.as_ref().map(|p| p.generate_arb_details(&self.dex_swaps)))
                    .collect(),

                gas_details:              tx_info.gas_details,
                swaps:                    self.dex_swaps,
                global_optimistic_end:    meta.microseconds_block_timestamp()
                    + config.optimistic_after_us,
                global_optimistic_start:  meta.microseconds_block_timestamp()
                    - config.optimistic_before_us,
                global_time_window_end:   meta.microseconds_block_timestamp()
                    + config.time_window_after_us,
                global_time_window_start: meta.microseconds_block_timestamp()
                    - config.time_window_before_us,
            }),
            self.max_profit
                .clone()
                .map(|v| {
                    v.arb_legs
                        .into_iter()
                        .flatten()
                        .map(|(_, v)| v)
                        .collect_vec()
                })
                .unwrap_or_default(),
        ))
    }

    pub fn arb_sanity_check(&self) -> ArbSanityCheck {
        let (profitable_exchanges_mid, profitable_exchanges_ask) = self
            .per_exchange_pnl
            .iter()
            .filter_map(|p| p.as_ref())
            .fold((Vec::new(), Vec::new()), |(mut mid, mut ask), p| {
                if p.aggregate_pnl.maker_taker_mid.0 > Rational::ZERO {
                    mid.push((
                        p.arb_legs[0].as_ref().unwrap().0.cex_quote.exchange,
                        p.aggregate_pnl.clone(),
                    ));
                }
                if p.aggregate_pnl.maker_taker_ask.0 > Rational::ZERO {
                    ask.push((
                        p.arb_legs[0].as_ref().unwrap().0.cex_quote.exchange,
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
#[derive(Debug, Clone, Default)]
pub struct PossibleCexDex {
    pub arb_legs:      Vec<Option<(ExchangeLeg, ExchangeLegCexPrice)>>,
    pub aggregate_pnl: ArbPnl,
}

impl PossibleCexDex {
    pub fn from_exchange_legs(
        exchange_legs: Vec<Option<(ExchangeLeg, ExchangeLegCexPrice)>>,
    ) -> Option<Self> {
        if exchange_legs.iter().all(Option::is_none) {
            return None
        }
        let mut total_mid_maker = Rational::ZERO;
        let mut total_mid_taker = Rational::ZERO;
        let mut total_ask_maker = Rational::ZERO;
        let mut total_ask_taker = Rational::ZERO;

        exchange_legs.iter().flatten().for_each(|(leg, _)| {
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
            .filter_map(|(index, details)| {
                let (leg, _) = details.as_ref()?;

                normalized_swaps.get(index).map(|swap| ArbDetails {
                    pairs:            leg.pairs.clone(),
                    trade_end_time:   leg.end_time,
                    trade_start_time: leg.start_time,
                    cex_exchange:     leg.cex_quote.exchange,
                    best_bid_maker:   leg.cex_quote.price_maker.0.clone(),
                    best_ask_maker:   leg.cex_quote.price_maker.1.clone(),
                    best_bid_taker:   leg.cex_quote.price_taker.0.clone(),
                    best_ask_taker:   leg.cex_quote.price_taker.1.clone(),
                    dex_exchange:     swap.protocol,
                    dex_price:        swap.swap_rate(),
                    dex_amount:       swap.amount_out.clone(),
                    pnl_pre_gas:      leg.pnl.clone(),
                })
            })
            .collect::<Vec<_>>()
    }
}

#[derive(Clone, Debug)]
pub struct ArbDetailsWithPrices {
    pub prices:  ExchangeLegCexPrice,
    pub details: ArbDetails,
}

impl fmt::Display for PossibleCexDex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", "Aggregate PnL:".bold().underline())?;
        writeln!(f, "  {}", self.aggregate_pnl)?;

        writeln!(f, "{}", "Arbitrage Legs:".bold().underline())?;
        if !self.arb_legs.is_empty() {
            for (index, leg) in self.arb_legs.iter().enumerate() {
                match leg {
                    Some(leg) => writeln!(f, "  - Leg {}: {}", index + 1, leg.0)?,
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
pub struct ExchangeLegCexPrice {
    pub token0: Address,
    pub price0: Rational,
    pub token1: Address,
    pub price1: Rational,
}

#[derive(Clone, Debug)]
pub struct ExchangeLeg {
    pub cex_quote:  FeeAdjustedQuote,
    pub start_time: u64,
    pub end_time:   u64,
    pub pairs:      Vec<Pair>,
    pub pnl:        ArbPnl,
}

pub fn log_price_delta(
    tx_hash: &FixedBytes<32>,
    token_in_symbol: &str,
    token_out_symbol: &str,
    dex_swap_rate: f64,
    cex_price: f64,
    token_in_address: &Address,
    token_out_address: &Address,
    price_calculation_type: PriceCalcType,
) {
    warn!(
        "\n\x1b[1;35mDetected significant price delta for direct pair for {} - {}:\x1b[0m\n\
         - \x1b[1;36mDEX Swap Rate:\x1b[0m {:.7}\n\
         - \x1b[1;36mCEX Price:\x1b[0m {:.7}\n\
         - Token Contracts:\n\
           * Token In: https://etherscan.io/address/{}\n\
           * Token Out: https://etherscan.io/address/{}\n
           * Tx Hash: https://etherscan.io/tx/{:?}\ngenerated_from: {price_calculation_type}",
        token_in_symbol,
        token_out_symbol,
        dex_swap_rate,
        cex_price,
        token_in_address,
        token_out_address,
        tx_hash
    );
}

#[derive(Debug)]
pub struct OptimisticDetails {
    pub optimistic_route_details: Vec<ArbDetails>,
    pub optimistic_trade_details: Vec<Vec<OptimisticTrade>>,
}
impl OptimisticDetails {
    pub fn route_pnl(&self) -> ArbPnl {
        let mut total_mid_maker = Rational::ZERO;
        let mut total_mid_taker = Rational::ZERO;
        let mut total_ask_maker = Rational::ZERO;
        let mut total_ask_taker = Rational::ZERO;

        self.optimistic_route_details.iter().for_each(|leg| {
            total_mid_maker += &leg.pnl_pre_gas.maker_taker_mid.0;
            total_mid_taker += &leg.pnl_pre_gas.maker_taker_mid.1;
            total_ask_maker += &leg.pnl_pre_gas.maker_taker_ask.0;
            total_ask_taker += &leg.pnl_pre_gas.maker_taker_ask.1;
        });

        ArbPnl {
            maker_taker_mid: (total_mid_maker, total_mid_taker),
            maker_taker_ask: (total_ask_maker, total_ask_taker),
        }
    }
}
