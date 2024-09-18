use brontes_types::{
    db::cex::CexExchange,
    mev::{BundleData, CexDexQuote},
    normalized_actions::NormalizedSwap,
    ToFloatNearest, TxInfo,
};
use malachite::{num::basic::traits::Zero, Rational};
use reth_primitives::Address;
use tracing::warn;

#[derive(Debug, Default)]
pub struct PossibleCexDex {
    pub arb_legs:      Vec<Option<ExchangeLeg>>,
    pub aggregate_pnl: f64,
    pub trade_prices:  Vec<ExchangeLegCexPrice>,
}

impl PossibleCexDex {
    pub fn from_exchange_legs(
        exchange_legs: Vec<Option<(ExchangeLeg, ExchangeLegCexPrice)>>,
    ) -> Option<Self> {
        let aggregate_pnl = exchange_legs
            .iter()
            .filter_map(|leg| leg.as_ref().map(|(el, _)| el.pnl))
            .sum();

        let (arb_legs, trade_prices): (Vec<_>, Vec<_>) = exchange_legs
            .into_iter()
            .map(|leg| {
                leg.map(|(el, price)| (Some(el), price))
                    .unwrap_or((None, ExchangeLegCexPrice::default()))
            })
            .unzip();

        Some(Self { arb_legs, aggregate_pnl, trade_prices })
    }

    pub fn adjust_for_gas_cost(&mut self, gas_cost: Rational) {
        self.aggregate_pnl -= gas_cost.to_float();
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
        t2_mid_price: Vec<f64>,
        t12_mid_price: Vec<f64>,
        t30_mid_price: Vec<f64>,
        t60_mid_price: Vec<f64>,
        t300_mid_price: Vec<f64>,
    ) -> Option<(f64, BundleData)> {
        Some((
            self.pnl.aggregate_pnl,
            BundleData::CexDexQuote(CexDexQuote {
                tx_hash: tx_info.tx_hash,
                block_number: tx_info.block_number,
                block_timestamp,
                instant_mid_price: self
                    .pnl
                    .arb_legs
                    .iter()
                    .map(|l| l.as_ref().unwrap_or(&ExchangeLeg::default()).cex_mid_price)
                    .collect(),
                t2_mid_price,
                t12_mid_price,
                t30_mid_price,
                t60_mid_price,
                t300_mid_price,
                pnl: self.pnl.aggregate_pnl,
                exchange: self.pnl.arb_legs[0].as_ref()?.exchange,
                gas_details: tx_info.gas_details,
                swaps: self.dex_swaps,
            }),
        ))
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
