use std::{fmt, fmt::Display, sync::Arc};

use alloy_primitives::FixedBytes;
use brontes_types::{
    db::cex::{config::CexDexTradeConfig, time_window_vwam::ExchangePath, CexExchange},
    mev::{ArbDetails, BundleData, CexDex, CexMethodology, OptimisticTrade},
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
    pub optimistic_details:  Option<OptimisticDetails>,
}

impl CexDexProcessing {
    pub fn new(
        dex_swaps: Vec<NormalizedSwap>,
        global_vmam_cex_dex: Option<PossibleCexDex>,
        per_exchange_pnl: Vec<Option<PossibleCexDex>>,
        optimistic_details: Option<OptimisticDetails>,
    ) -> Option<Self> {
        let mut this = Self {
            per_exchange_pnl,
            dex_swaps,
            max_profit: None,
            global_vmam_cex_dex,
            optimistic_details,
        };
        this.construct_max_profit_route()?;
        Some(this)
    }

    pub fn construct_max_profit_route(&mut self) -> Option<()> {
        if self.per_exchange_pnl.iter().all(Option::is_none) {
            return None;
        }

        let num_legs = self.dex_swaps.len();
        let mut best_legs: Vec<Option<ArbLeg>> = vec![None; num_legs];
        let mut aggregate_pnl_maker = Rational::ZERO;
        let mut aggregate_pnl_taker = Rational::ZERO;

        for possible_cex_dex in self.per_exchange_pnl.iter().flatten() {
            for (i, arb_leg) in possible_cex_dex.arb_legs.iter().enumerate() {
                if let Some(leg) = arb_leg {
                    let current_pnl = &leg.pnl_maker;
                    let best_pnl = best_legs[i]
                        .as_ref()
                        .map_or(Rational::ZERO, |best| best.pnl_maker);

                    if current_pnl > &best_pnl {
                        best_legs[i] = Some(leg.clone());
                        aggregate_pnl_maker += &leg.pnl_maker
                            - best_legs[i]
                                .as_ref()
                                .map_or(Rational::ZERO, |l| l.pnl_maker);
                        aggregate_pnl_taker += &leg.pnl_taker
                            - best_legs[i]
                                .as_ref()
                                .map_or(Rational::ZERO, |l| l.pnl_taker);
                    }
                }
            }
        }

        self.max_profit =
            Some(PossibleCexDex { arb_legs: best_legs, aggregate_pnl_maker, aggregate_pnl_taker });

        self.per_exchange_pnl.retain(|possible_cex_dex| {
            possible_cex_dex
                .as_ref()
                .map_or(false, |cex_dex| cex_dex.arb_legs.iter().all(Option::is_some))
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
            .optimistic_details
            .as_ref()
            .map(|o| o.aggregate_pnl_maker.clone());

        let window = self
            .global_vmam_cex_dex
            .as_ref()
            .map(|w| w.aggregate_pnl_maker.clone());

        let max_profit = self
            .max_profit
            .as_ref()
            .map(|v| v.aggregate_pnl_maker.clone());

        let (header_pnl, header_pnl_methodology) = [
            (max_profit, CexMethodology::OptimalRouteVWAP),
            (optimistic, CexMethodology::Optimistic),
            (window, CexMethodology::GlobalWWAP),
        ]
        .into_iter()
        .filter_map(|(pnl, methodology)| pnl.map(|p| (p, methodology)))
        .max_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or_else(|| {
            (
                self.max_profit
                    .as_ref()
                    .expect(
                        "Max profit should always exist, CexDex inspector should have returned \
                         early",
                    )
                    .aggregate_pnl_maker
                    .clone(),
                CexMethodology::OptimalRouteVWAP,
            )
        });

        Some((
            header_pnl.to_float(),
            BundleData::CexDex(CexDex {
                block_number: meta.block_num,
                block_timestamp: meta.microseconds_block_timestamp(),
                tx_hash: tx_info.tx_hash,
                header_pnl_methodology,
                global_vmap_pnl_maker: self
                    .global_vmam_cex_dex
                    .as_ref()
                    .map_or(Rational::ZERO, |v| v.aggregate_pnl_maker.clone())
                    .to_float(),

                global_vmap_pnl_taker: self
                    .global_vmam_cex_dex
                    .as_ref()
                    .map_or(Rational::ZERO, |v| v.aggregate_pnl_taker.clone())
                    .to_float(),

                global_vmap_details: self
                    .global_vmam_cex_dex?
                    .generate_arb_details(&self.dex_swaps),

                optimal_route_details: self
                    .max_profit
                    .as_ref()?
                    .generate_arb_details(&self.dex_swaps),

                optimal_route_pnl_maker: self
                    .max_profit
                    .as_ref()
                    .map(|v| v.aggregate_pnl_maker.clone().to_float())
                    .unwrap_or_default(),
                optimal_route_pnl_taker: self
                    .max_profit
                    .as_ref()
                    .map(|v| v.aggregate_pnl_taker.clone().to_float())
                    .unwrap_or_default(),

                per_exchange_pnl: self
                    .per_exchange_pnl
                    .iter()
                    .filter_map(|p| {
                        p.as_ref().and_then(|p| {
                            p.arb_legs.first().and_then(|leg| {
                                leg.as_ref().map(|leg| {
                                    (
                                        leg.exchange,
                                        (
                                            p.aggregate_pnl_maker.clone().to_float(),
                                            p.aggregate_pnl_taker.clone().to_float(),
                                        ),
                                    )
                                })
                            })
                        })
                    })
                    .collect(),

                optimistic_route_details: self
                    .optimistic_details
                    .as_ref()
                    .map(|r| r.generate_arb_details(&self.dex_swaps))
                    .unwrap_or_default(),

                optimistic_trade_details: self
                    .optimistic_details
                    .as_ref()
                    .map(|r| r.trade_details.to_vec())
                    .unwrap_or_default(),

                optimistic_route_pnl_maker: self
                    .optimistic_details
                    .as_ref()
                    .map_or(0.0, |r| r.aggregate_pnl_maker.to_float()),

                optimistic_route_pnl_taker: self
                    .optimistic_details
                    .as_ref()
                    .map_or(0.0, |r| r.aggregate_pnl_taker.to_float()),

                per_exchange_details: self
                    .per_exchange_pnl
                    .iter()
                    .filter_map(|p| p.as_ref().map(|p| p.generate_arb_details(&self.dex_swaps)))
                    .collect(),

                gas_details: tx_info.gas_details,
                swaps: self.dex_swaps,
            }),
            self.max_profit
                .clone()
                .map(|v| {
                    v.arb_legs
                        .into_iter()
                        .flatten()
                        .map(|v| v.token_price)
                        .collect_vec()
                })
                .unwrap_or_default(),
        ))
    }

    pub fn arb_sanity_check(&self) -> ArbSanityCheck {
        let (profitable_exchanges_maker, profitable_exchanges_taker) = self
            .per_exchange_pnl
            .iter()
            .filter_map(|p| p.as_ref())
            .fold((Vec::new(), Vec::new()), |(mut mid, mut ask), p| {
                if p.aggregate_pnl_maker > Rational::ZERO {
                    mid.push((
                        p.arb_legs[0].as_ref().unwrap().exchange,
                        p.aggregate_pnl_maker.clone(),
                    ));
                }
                if p.aggregate_pnl_taker > Rational::ZERO {
                    ask.push((
                        p.arb_legs[0].as_ref().unwrap().exchange,
                        p.aggregate_pnl_taker.clone(),
                    ));
                }
                (mid, ask)
            });

        let profitable_cross_exchange = {
            let maker_price_profitability = self
                .max_profit
                .as_ref()
                .expect(
                    "Max profit should always exist, CexDex inspector should have returned early",
                )
                .aggregate_pnl_maker
                > Rational::ZERO;

            let taker_price_profitability =
                self.max_profit.as_ref().unwrap().aggregate_pnl_maker > Rational::ZERO;

            (maker_price_profitability, taker_price_profitability)
        };

        let global_profitability =
            self.global_vmam_cex_dex
                .as_ref()
                .map_or((false, false), |global| {
                    (
                        global.aggregate_pnl_maker > Rational::ZERO,
                        global.aggregate_pnl_taker > Rational::ZERO,
                    )
                });

        let is_stable_swaps = self.is_stable_swaps();

        ArbSanityCheck {
            profitable_exchanges_maker,
            profitable_exchanges_taker,
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
    pub arb_legs:            Vec<Option<ArbLeg>>,
    pub aggregate_pnl_maker: Rational,
    pub aggregate_pnl_taker: Rational,
}

impl PossibleCexDex {
    pub fn from_arb_legs(arb_legs: Vec<Option<ArbLeg>>) -> Option<Self> {
        if arb_legs.iter().all(Option::is_none) {
            return None;
        }

        let mut aggregate_pnl_maker = Rational::ZERO;
        let mut aggregate_pnl_taker = Rational::ZERO;

        arb_legs.iter().flatten().for_each(|leg| {
            aggregate_pnl_maker += &leg.pnl_maker;
            aggregate_pnl_taker += &leg.pnl_taker;
        });

        Some(PossibleCexDex { arb_legs, aggregate_pnl_maker, aggregate_pnl_taker })
    }

    pub fn adjust_for_gas_cost(&mut self, gas_cost: &Rational) {
        self.aggregate_pnl_maker -= gas_cost;
        self.aggregate_pnl_taker -= gas_cost;
    }

    pub fn generate_arb_details(&self, normalized_swaps: &[NormalizedSwap]) -> Vec<ArbDetails> {
        self.arb_legs
            .iter()
            .enumerate()
            .filter_map(|(index, arb_leg)| {
                let leg = arb_leg.as_ref()?;
                normalized_swaps.get(index).map(|swap| ArbDetails {
                    pairs:            leg.pairs.clone(),
                    trade_end_time:   leg.price.final_end_time,
                    trade_start_time: leg.price.final_start_time,
                    cex_exchange:     leg.exchange,
                    price_maker:      leg.price.price_maker.clone(),
                    price_taker:      leg.price.price_taker.clone(),
                    dex_exchange:     swap.protocol,
                    dex_price:        swap.swap_rate(),
                    dex_amount:       swap.amount_out.clone(),
                    pnl_maker:        leg.pnl_maker.clone(),
                    pnl_taker:        leg.pnl_taker.clone(),
                })
            })
            .collect::<Vec<_>>()
    }
}

#[derive(Debug, Clone)]
pub struct ArbLeg {
    pub price:       ExchangePath,
    pub exchange:    CexExchange,
    pub pnl_maker:   Rational,
    pub pnl_taker:   Rational,
    pub pairs:       Vec<Pair>,
    pub token_price: ExchangeLegCexPrice,
}

impl ArbLeg {
    pub fn new(
        price: ExchangePath,
        exchange: CexExchange,
        pnl_maker: Rational,
        pnl_taker: Rational,
        pairs: Vec<Pair>,
        token_price: ExchangeLegCexPrice,
    ) -> Self {
        Self { price, exchange, pnl_maker, pnl_taker, pairs, token_price }
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
        writeln!(f, "  {}", self.aggregate_pnl_maker)?;

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
    pub profitable_exchanges_maker: Vec<(CexExchange, Rational)>,
    pub profitable_exchanges_taker: Vec<(CexExchange, Rational)>,
    pub profitable_cross_exchange:  (bool, bool),
    pub global_profitability:       (bool, bool),
    pub is_stable_swaps:            bool,
}

impl ArbSanityCheck {
    /// Determines if the CEX-DEX arbitrage is a highly profitable outlier.
    ///
    /// This function checks if the arbitrage is only profitable on a single
    /// exchange based on the taker price, and if the profit on this exchange
    /// exceeds a high profit threshold (e.g., $10,000). Additionally, it
    /// verifies if the exchange is either Kucoin or Okex.
    ///
    /// Returns `true` if all conditions are met, indicating a highly profitable
    /// outlier.
    pub fn is_profitable_outlier(&self) -> bool {
        !self.profitable_exchanges_taker.is_empty()
            && self.profitable_exchanges_taker.len() == 1
            && self.profitable_exchanges_taker[0].1 > HIGH_PROFIT_THRESHOLD
            && (self.profitable_exchanges_taker[0].0 == CexExchange::Kucoin
                || self.profitable_exchanges_taker[0].0 == CexExchange::Okex)
    }
}

impl fmt::Display for ArbSanityCheck {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "\x1b[1m\x1b[4mCex Dex Sanity Check\x1b[0m\x1b[24m")?;
        writeln!(f, "Profitable Exchanges Based on Maker Price:")?;
        for (index, (exchange, pnl)) in self.profitable_exchanges_maker.iter().enumerate() {
            writeln!(f, "    - Exchange {}: {}", index + 1, exchange)?;
            writeln!(f, "        - ARB PNL: {}", pnl)?;
        }

        writeln!(f, "Profitable Exchanges Based on Taker Price:")?;
        for (index, (exchange, pnl)) in self.profitable_exchanges_taker.iter().enumerate() {
            writeln!(f, "    - Exchange {}: {}", index + 1, exchange)?;
            writeln!(f, "        - ARB PNL: {}", pnl)?;
        }

        writeln!(
            f,
            "Is profitable cross exchange (Maker Price): {}",
            if self.profitable_cross_exchange.0 { "Yes" } else { "No" }
        )?;
        writeln!(
            f,
            "Is profitable cross exchange (Taker Price): {}",
            if self.profitable_cross_exchange.1 { "Yes" } else { "No" }
        )?;

        writeln!(
            f,
            "Is globally profitable based on cross exchange VMAP (Maker Price): {}",
            if self.global_profitability.0 { "Yes" } else { "No" }
        )?;
        writeln!(
            f,
            "Is globally profitable based on cross exchange VMAP (Taker Price): {}",
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

#[derive(Clone, Debug)]
pub struct ExchangeLegCexPrice {
    pub token0: Address,
    pub price0: Rational,
    pub token1: Address,
    pub price1: Rational,
}

pub fn log_cex_trade_price_delta(
    tx_hash: &FixedBytes<32>,
    token_in_symbol: &str,
    token_out_symbol: &str,
    dex_swap_rate: f64,
    cex_price: f64,
    token_in_address: &Address,
    token_out_address: &Address,
    price_calculation_type: PriceCalcType,
    dex_amount_in: &Rational,
    dex_amount_out: &Rational,
    cex_output: &Rational,
) {
    let mut arb_ratio = Rational::ZERO;
    if dex_amount_in != &Rational::ZERO {
        arb_ratio = cex_output.clone() / dex_amount_in;
    }

    let arb_percent = (arb_ratio.clone().to_float() - 1.0) * 100.0;

    warn!(
        "\n\x1b[1;35mSignificant CEX trade price discrepancy detected for {} - {}:\x1b[0m\n\
         - \x1b[1;36mDEX Swap:\x1b[0m\n\
           * Rate: {:.7}\n\
           * Amount In: {}\n\
           * Amount Out: {}\n\
         - \x1b[1;36mCEX Trade:\x1b[0m\n\
           * Rate: {:.7}\n\
           * Equivalent Output: {}\n\
         - \x1b[1;33mArbitrage Ratio:\x1b[0m {:.4} ({}%)\n\
         - Token Contracts:\n\
           * Token In: https://etherscan.io/address/{}\n\
           * Token Out: https://etherscan.io/address/{}\n\
         - Tx Hash: https://etherscan.io/tx/{:?}\n\
         - Price Calculation Type: {}\n\
         - \x1b[1;31mWarning:\x1b[0m The CEX trade output is more than 2x the DEX input, indicating a potentially invalid trade or extreme market inefficiency.",
        token_in_symbol,
        token_out_symbol,
        dex_swap_rate,
        dex_amount_in.clone().to_float(),
        dex_amount_out.clone().to_float(),
        cex_price,
        cex_output.clone().to_float(),
        arb_ratio.to_float(),
        arb_percent,
        token_in_address,
        token_out_address,
        tx_hash,
        price_calculation_type
    );
}

#[derive(Debug)]
pub struct OptimisticDetails {
    pub arb_legs:            Vec<Option<ArbLeg>>,
    pub trade_details:       Vec<Vec<OptimisticTrade>>,
    pub aggregate_pnl_maker: Rational,
    pub aggregate_pnl_taker: Rational,
}

impl OptimisticDetails {
    pub fn new(arb_legs: Vec<Option<ArbLeg>>, trade_details: Vec<Vec<OptimisticTrade>>) -> Self {
        let mut details = Self {
            arb_legs,
            trade_details,
            aggregate_pnl_maker: Rational::ZERO,
            aggregate_pnl_taker: Rational::ZERO,
        };
        details.calculate_and_store_aggregate_pnl();
        details
    }

    fn calculate_and_store_aggregate_pnl(&mut self) {
        let (maker_pnl, taker_pnl) = self
            .arb_legs
            .iter()
            .flatten()
            .fold((Rational::ZERO, Rational::ZERO), |(maker_sum, taker_sum), leg| {
                (maker_sum + &leg.pnl_maker, taker_sum + &leg.pnl_taker)
            });

        self.aggregate_pnl_maker = maker_pnl;
        self.aggregate_pnl_taker = taker_pnl;
    }

    pub fn generate_arb_details(&self, normalized_swaps: &[NormalizedSwap]) -> Vec<ArbDetails> {
        self.arb_legs
            .iter()
            .enumerate()
            .filter_map(|(index, arb_leg)| {
                let leg = arb_leg.as_ref()?;
                normalized_swaps.get(index).map(|swap| ArbDetails {
                    pairs:            leg.pairs.clone(),
                    trade_end_time:   leg.price.final_end_time,
                    trade_start_time: leg.price.final_start_time,
                    cex_exchange:     leg.exchange,
                    price_maker:      leg.price.price_maker.clone(),
                    price_taker:      leg.price.price_taker.clone(),
                    dex_exchange:     swap.protocol,
                    dex_price:        swap.swap_rate(),
                    dex_amount:       swap.amount_out.clone(),
                    pnl_maker:        leg.pnl_maker.clone(),
                    pnl_taker:        leg.pnl_taker.clone(),
                })
            })
            .collect::<Vec<_>>()
    }
}
