use alloy_primitives::Address;
use brontes_types::{
    db::cex::CexExchange,
    mev::{BundleData, CexDexQuote},
    normalized_actions::NormalizedSwap,
    ToFloatNearest, TxInfo,
};
use malachite::{num::basic::traits::Zero, Rational};
use tracing::warn;

#[derive(Debug, Default)]
pub struct PossibleCexDex {
    pub arb_legs:       Vec<Vec<Option<ExchangeLeg>>>,
    pub aggregate_pnls: Vec<f64>,
    pub trade_prices:   Vec<Vec<ExchangeLegCexPrice>>,
}

impl PossibleCexDex {
    pub fn from_exchange_legs(
        exchange_legs: Vec<Vec<Option<(ExchangeLeg, ExchangeLegCexPrice)>>>,
    ) -> Option<Self> {
        let mut arb_legs = Vec::with_capacity(exchange_legs.len());
        let mut trade_prices = Vec::with_capacity(exchange_legs.len());
        let mut aggregate_pnls = Vec::with_capacity(exchange_legs.len());

        for markout_time in exchange_legs {
            let mut this_arb = Vec::with_capacity(markout_time.len());
            let mut this_prices = Vec::with_capacity(markout_time.len());
            let mut this_pnl_sum = 0.0f64;

            for opt in markout_time {
                match opt {
                    Some((leg, price)) => {
                        this_pnl_sum += leg.pnl;
                        this_arb.push(Some(leg));
                        this_prices.push(price);
                    }
                    None => {
                        this_arb.push(None);
                        this_prices.push(ExchangeLegCexPrice::default());
                    }
                }
            }

            arb_legs.push(this_arb);
            trade_prices.push(this_prices);
            aggregate_pnls.push(this_pnl_sum);
        }

        Some(Self { arb_legs, aggregate_pnls, trade_prices })
    }

    pub fn adjust_for_gas_cost(&mut self, gas_cost: Rational) {
        let gas_cost_float = gas_cost.to_float();
        for pnl in &mut self.aggregate_pnls {
            *pnl -= gas_cost_float;
        }
    }

    pub fn extend(&mut self, other: PossibleCexDex) {
        self.arb_legs.extend(other.arb_legs);
        self.aggregate_pnls.extend(other.aggregate_pnls);
        self.trade_prices.extend(other.trade_prices);
    }
}

#[derive(Debug)]
pub struct CexDexProcessing {
    pub dex_swaps: Vec<NormalizedSwap>,
    pub pnl:       PossibleCexDex,
}

impl CexDexProcessing {
    pub fn into_bundle(
        self,
        tx_info: &TxInfo,
        block_timestamp: u64,
        tx_cost: f64,
    ) -> Option<(f64, BundleData)> {
        Some((
            self.pnl.aggregate_pnls[0],
            BundleData::CexDexQuote(CexDexQuote {
                tx_hash: tx_info.tx_hash,
                block_number: tx_info.block_number,
                block_timestamp,
                exchange: self.pnl.arb_legs[0][0].as_ref()?.exchange,
                swaps: self.dex_swaps,
                t0_mid_price: self.pnl.arb_legs[0]
                    .iter()
                    .map(|l| l.as_ref().unwrap_or(&ExchangeLeg::default()).cex_mid_price)
                    .collect(),
                t2_mid_price: self.pnl.arb_legs[1]
                    .iter()
                    .map(|l| l.as_ref().unwrap_or(&ExchangeLeg::default()).cex_mid_price)
                    .collect(),
                t6_mid_price: self.pnl.arb_legs[2]
                    .iter()
                    .map(|l| l.as_ref().unwrap_or(&ExchangeLeg::default()).cex_mid_price)
                    .collect(),
                t12_mid_price: self.pnl.arb_legs[3]
                    .iter()
                    .map(|l| l.as_ref().unwrap_or(&ExchangeLeg::default()).cex_mid_price)
                    .collect(),
                t30_mid_price: self.pnl.arb_legs[4]
                    .iter()
                    .map(|l| l.as_ref().unwrap_or(&ExchangeLeg::default()).cex_mid_price)
                    .collect(),
                t60_mid_price: self.pnl.arb_legs[5]
                    .iter()
                    .map(|l| l.as_ref().unwrap_or(&ExchangeLeg::default()).cex_mid_price)
                    .collect(),
                t300_mid_price: self.pnl.arb_legs[6]
                    .iter()
                    .map(|l| l.as_ref().unwrap_or(&ExchangeLeg::default()).cex_mid_price)
                    .collect(),
                t0_pnl: self.pnl.aggregate_pnls[0],
                t2_pnl: self.pnl.aggregate_pnls[1],
                t6_pnl: self.pnl.aggregate_pnls[2],
                t12_pnl: self.pnl.aggregate_pnls[3],
                t30_pnl: self.pnl.aggregate_pnls[4],
                t60_pnl: self.pnl.aggregate_pnls[5],
                t300_pnl: self.pnl.aggregate_pnls[6],
                gas_details: tx_info.gas_details,
                tx_cost,
            }),
        ))
    }

    pub fn extend(&mut self, other: PossibleCexDex) {
        self.pnl.extend(other);
    }
}

#[derive(Clone, Debug, Default)]
pub struct ExchangeLeg {
    pub cex_mid_price: f64,
    pub pnl:           f64,
    pub exchange:      CexExchange,
}

pub fn log_cex_dex_quote_delta(
    tx_hash: &str,
    token_in_symbol: &str,
    token_out_symbol: &str,
    exchange: &CexExchange,
    dex_swap_rate: f64,
    cex_price: f64,
    token_in_address: &Address,
    token_out_address: &Address,
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
        "\n\x1b[1;35mSignificant Cex-Dex quote discrepancy detected for {} - {} on {}:\x1b[0m\n\
         - \x1b[1;36mDEX Swap:\x1b[0m\n\
           * Rate: {:.7}\n\
           * Amount In: {}\n\
           * Amount Out: {}\n\
         - \x1b[1;36mCEX Quote:\x1b[0m\n\
           * Rate: {:.7}\n\
           * Equivalent Output: {}\n\
         - \x1b[1;33mArbitrage Ratio:\x1b[0m {:.4} ({}%)\n\
         - Token Contracts:\n\
           * Token In: https://etherscan.io/address/{}\n\
           * Token Out: https://etherscan.io/address/{}\n\
         - Tx Hash: https://etherscan.io/tx/{}\n\
         - \x1b[1;31mWarning:\x1b[0m The CEX quote output is more than 2x the DEX input, suggesting a potentially invalid quote or extreme market inefficiency.",
        token_in_symbol,
        token_out_symbol,
        exchange,
        dex_swap_rate,
        dex_amount_in.clone().to_float(),
        dex_amount_out.clone().to_float(),
        cex_price,
        cex_output.clone().to_float(),
        arb_ratio.to_float(),
        arb_percent,
        token_in_address,
        token_out_address,
        tx_hash
    );
}

#[derive(Clone, Debug, Default)]
pub struct ExchangeLegCexPrice {
    pub token0: Address,
    pub price0: Rational,
    pub token1: Address,
    pub price1: Rational,
}
