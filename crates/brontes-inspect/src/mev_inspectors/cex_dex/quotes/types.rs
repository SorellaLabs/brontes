use brontes_types::{
    db::cex::CexExchange,
    mev::{BundleData, CexDexQuote},
    normalized_actions::NormalizedSwap,
    ToFloatNearest, TxInfo,
};
use malachite::Rational;
use reth_primitives::Address;
use tracing::error;

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
    pub fn into_bundle(self, tx_info: &TxInfo, block_timestamp: u64) -> Option<(f64, BundleData)> {
        Some((
            self.pnl.aggregate_pnl,
            BundleData::CexDex(CexDexQuote {
                tx_hash: tx_info.tx_hash,
                block_number: tx_info.block_number,
                block_timestamp,
                mid_price: self
                    .pnl
                    .arb_legs
                    .iter()
                    .map(|l| l.as_ref().unwrap_or(&ExchangeLeg::default()).cex_mid_price)
                    .collect(),
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

pub fn log_price_delta(
    tx_hash: String,
    token_in_symbol: &str,
    token_out_symbol: &str,
    exchange: &CexExchange,
    dex_swap_rate: f64,
    cex_price: f64,
    token_in_address: &Address,
    token_out_address: &Address,
) {
    error!(
        "\n\x1b[1;35mDetected significant price delta for direct pair for {} - {} on {}:\x1b[0m\n\
         - \x1b[1;36mDEX Swap Rate:\x1b[0m {:.7}\n\
         - \x1b[1;36mCEX Price:\x1b[0m {:.7}\n\
         - Token Contracts:\n\
           * Token In: https://etherscan.io/address/{}\n\
           * Token Out: https://etherscan.io/address/{}\n\
           * Tx Hash: https://etherscan.io/tx/{}\n",
        token_in_symbol,
        token_out_symbol,
        exchange,
        dex_swap_rate,
        cex_price,
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
