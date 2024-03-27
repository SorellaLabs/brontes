use std::sync::Arc;

use brontes_database::libmdbx::LibmdbxReader;
use brontes_types::{
    db::{cex::CexExchange, cex_trades::ExchangePrice, dex::PriceAt},
    mev::{Bundle, BundleData, CexDex, MevType, StatArbDetails, StatArbPnl},
    normalized_actions::{accounting::ActionAccounting, Actions, NormalizedSwap},
    pair::Pair,
    tree::{BlockTree, GasDetails},
    ActionIter, ToFloatNearest, TreeSearchBuilder, TxInfo,
};
use itertools::Itertools;
use malachite::{num::basic::traits::Zero, Rational};
use reth_primitives::Address;

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
                let deltas = swaps.clone().into_iter().account_for_actions();
                let swaps = swaps
                    .into_iter()
                    .collect_action_vec(Actions::try_swaps_merged);

                // For each swap in the transaction, detect potential CEX-DEX
                let cex_dex_legs: Vec<PossibleCexDexLeg> = swaps
                    .into_iter()
                    .filter_map(|swap| self.detect_cex_dex_opportunity(&swap, metadata.as_ref()))
                    .collect();

                let possible_cex_dex =
                    self.gas_accounting(cex_dex_legs, &tx_info.gas_details, metadata.clone())?;

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
                );

                Some(Bundle { header, data: cex_dex })
            })
            .collect::<Vec<_>>()
    }
}

impl<DB: LibmdbxReader> CexDexMarkoutInspector<'_, DB> {
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
        swap: &NormalizedSwap,
        metadata: &Metadata,
    ) -> Option<PossibleCexDexLeg> {
        let pair = Pair(swap.token_out.address, swap.token_in.address);

        let (maker_price, taker_price) = metadata.cex_trades.as_ref()?.get_price(
            &self.cex_exchanges,
            &pair,
            // we always are buying amount in on cex
            &swap.amount_in,
            // arbitrary for now
            25,
            // add lookup
            None,
        )?;
        let leg = self.profit_classifier(swap, maker_price, taker_price);

        Some(PossibleCexDexLeg { swap: swap.clone(), leg })
    }

    /// For a given swap & CEX quote, calculates the potential profit from
    /// buying on DEX and selling on CEX. This function also accounts for CEX
    /// trading fees.
    fn profit_classifier(
        &self,
        swap: &NormalizedSwap,
        maker_price: ExchangePrice,
        taker_price: ExchangePrice,
    ) -> SwapLeg {
        // A positive delta indicates potential profit from buying on DEX
        // and selling on CEX.
        let rate = swap.swap_rate();
        let maker_delta = &maker_price.price - &rate;
        let taker_delta = &taker_price.price - &rate;

        let (maker_profit, taker_profit) = (
            // prices are fee adjusted already so no need to calcuate taking a fee here
            maker_delta * &swap.amount_out * &maker_price.price,
            taker_delta * &swap.amount_out * &taker_price.price,
        );

        SwapLeg { taker_price, maker_price, pnl: StatArbPnl { maker_profit, taker_profit } }
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
                let most_profitable_leg = &swap_with_profit.leg;

                swaps.push(swap_with_profit.swap.clone());
                arb_details.push(StatArbDetails {
                    cex_exchanges: most_profitable_leg
                        .maker_price
                        .exchanges
                        .iter()
                        .map(|(a, _)| *a)
                        .collect_vec(),
                    cex_price:     most_profitable_leg.maker_price.price.clone(),
                    dex_exchange:  swap_with_profit.swap.protocol,
                    dex_price:     swap_with_profit.swap.swap_rate(),
                    pnl_pre_gas:   most_profitable_leg.pnl.clone(),
                });

                total_arb_pre_gas.maker_profit += &most_profitable_leg.pnl.maker_profit;
                total_arb_pre_gas.taker_profit += &most_profitable_leg.pnl.taker_profit;
            });

        if swaps.is_empty() {
            return None
        }

        let gas_cost = metadata.get_gas_price_usd(gas_details.gas_paid(), self.utils.quote);

        let pnl = StatArbPnl {
            maker_profit: total_arb_pre_gas.maker_profit - &gas_cost,
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
                && (possible_cex_dex.gas_details.coinbase_transfer.is_some() && info.is_private
                    || info.is_cex_dex_call))
            || info.is_searcher_of_type(MevType::CexDex)
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

pub struct PossibleCexDexLeg {
    pub swap: NormalizedSwap,
    pub leg:  SwapLeg,
}

#[derive(Clone)]
pub struct SwapLeg {
    pub maker_price: ExchangePrice,
    pub taker_price: ExchangePrice,
    pub pnl:         StatArbPnl,
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
    async fn test_cex_dex_markout() {
        let inspector_util = InspectorTestUtils::new(USDT_ADDRESS, 0.5).await;

        let tx = hex!("21b129d221a4f169de0fc391fe0382dbde797b69300a9a68143487c54d620295").into();

        let config = InspectorTxRunConfig::new(Inspectors::CexDexMarkout)
            .with_mev_tx_hashes(vec![tx])
            .with_dex_prices()
            .with_expected_profit_usd(6772.69)
            .with_gas_paid_usd(78993.39);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_eoa_cex_dex_markout() {
        let inspector_util = InspectorTestUtils::new(USDT_ADDRESS, 0.5).await;

        let tx = hex!("dfe3152caaf92e5a9428827ea94eff2a822ddcb22129499da4d5b6942a7f203e").into();

        let config = InspectorTxRunConfig::new(Inspectors::CexDexMarkout)
            .with_mev_tx_hashes(vec![tx])
            .with_dex_prices()
            .with_expected_profit_usd(7201.40)
            .with_gas_paid_usd(6261.08);

        inspector_util.run_inspector(config, None).await.unwrap();
    }
}
