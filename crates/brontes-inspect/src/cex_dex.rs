use std::sync::Arc;

use brontes_database::Metadata;
use brontes_types::{
    classified_mev::{CexDex, MevType, SpecificMev},
    normalized_actions::{Actions, NormalizedSwap},
    tree::{GasDetails, TimeTree},
    ToScaledRational, TOKEN_TO_DECIMALS,
};
use malachite::{
    num::{basic::traits::Zero, conversion::traits::RoundingFrom},
    rounding_modes::RoundingMode,
    Rational,
};
use rayon::{
    iter::{IntoParallelIterator, ParallelIterator},
    prelude::IntoParallelRefIterator,
};
use reth_primitives::{Address, H256};
use tracing::error;

use crate::{ClassifiedMev, Inspector};

pub struct CexDexInspector;

impl CexDexInspector {
    fn process_swap(
        &self,
        hash: H256,
        mev_contract: Address,
        eoa: Address,
        metadata: Arc<Metadata>,
        gas_details: &GasDetails,
        swaps: Vec<Vec<Actions>>,
    ) -> Option<(ClassifiedMev, Box<dyn SpecificMev>)> {
        let deltas = self.calculate_swap_deltas(&swaps);

        let mev_profit_collector = self
            .get_best_usd_delta(
                deltas.clone(),
                metadata.clone(),
                Box::new(|(appearance, _)| appearance),
            )?
            .0;

        let (swap_data, (pre, post)): (Vec<Actions>, _) = swaps
            .into_iter()
            .flatten()
            .filter_map(|action| {
                if let Actions::Swap(ref normalized_swap) = action {
                    let (pre, post) = self.get_cex_dex(normalized_swap, metadata.as_ref());
                    Some((action, (pre, post)))
                } else {
                    None
                }
            })
            .unzip();

        let profit_pre = self.arb_gas(pre, gas_details, &metadata.eth_prices.0)?;
        let profit_post = self.arb_gas(post, gas_details, &metadata.eth_prices.1)?;

        let classified = ClassifiedMev {
            mev_profit_collector,
            tx_hash: hash,
            mev_contract,
            eoa,
            block_number: metadata.block_num,
            mev_type: MevType::CexDex,
            submission_profit_usd: f64::rounding_from(profit_pre, RoundingMode::Nearest).0,
            finalized_profit_usd: f64::rounding_from(profit_post, RoundingMode::Nearest).0,
            submission_bribe_usd: f64::rounding_from(
                Rational::from(gas_details.gas_paid()) * &metadata.eth_prices.1,
                RoundingMode::Nearest,
            )
            .0,
            finalized_bribe_usd: f64::rounding_from(
                Rational::from(gas_details.gas_paid()) * &metadata.eth_prices.1,
                RoundingMode::Nearest,
            )
            .0,
        };

        let (dex_prices, cex_prices) = swap_data
            .par_iter()
            .filter_map(|swap| self.rational_dex_price(swap, &metadata))
            .map(|(dex_price, _, cex1)| {
                (
                    f64::rounding_from(dex_price, RoundingMode::Nearest).0,
                    f64::rounding_from(cex1, RoundingMode::Nearest).0,
                )
            })
            .unzip();

        let cex_dex = CexDex {
            tx_hash: hash,
            gas_details: gas_details.clone(),
            swaps: swap_data,
            cex_prices,
            dex_prices,
        };

        Some((classified, Box::new(cex_dex)))
    }

    fn arb_gas(
        &self,
        arbs: Vec<Option<Rational>>,
        gas_details: &GasDetails,
        eth_price: &Rational,
    ) -> Option<Rational> {
        Some(
            arbs.into_iter().flatten().sum::<Rational>()
                - Rational::from(gas_details.gas_paid()) * eth_price,
        )
        .filter(|p| p > &Rational::ZERO)
    }

    pub fn get_cex_dex(
        &self,
        swap: &NormalizedSwap,
        metadata: &Metadata,
    ) -> (Option<Rational>, Option<Rational>) {
        self.rational_dex_price(&Actions::Swap(swap.clone()), metadata)
            .map(|(dex_price, cex_price1, cex_price2)| {
                let profit1 = self.profit_classifier(swap, &dex_price, &cex_price1);
                let profit2 = self.profit_classifier(swap, &dex_price, &cex_price2);

                (
                    Some(profit1).filter(|p| Rational::ZERO.lt(p)),
                    Some(profit2).filter(|p| Rational::ZERO.lt(p)),
                )
            })
            .unwrap_or((None, None))
    }

    fn profit_classifier(
        &self,
        swap: &NormalizedSwap,
        dex_price: &Rational,
        cex_price: &Rational,
    ) -> Rational {
        // Calculate the price differences between DEX and CEX
        let delta_price = cex_price - dex_price;

        // Calculate the potential profit
        delta_price * swap.amount_in.to_scaled_rational(18)
    }

    pub fn rational_dex_price(
        &self,
        swap: &Actions,
        metadata: &Metadata,
    ) -> Option<(Rational, Rational, Rational)> {
        let Actions::Swap(swap) = swap else { return None };

        let Some(decimals_in) = TOKEN_TO_DECIMALS.get(&swap.token_in.0) else {
            error!(missing_token=?swap.token_in, "missing token in token to decimal map");
            return None
        };
        //TODO(JOE): this is ugly asf, but we should have some metrics shit so we can
        // log it
        let Some(decimals_out) = TOKEN_TO_DECIMALS.get(&swap.token_out.0) else {
            error!(missing_token=?swap.token_in, "missing token in token to decimal map");
            return None
        };

        let adjusted_in = swap.amount_in.to_scaled_rational(*decimals_in);
        let adjusted_out = swap.amount_out.to_scaled_rational(*decimals_out);

        let centralized_prices = metadata.token_prices.get(&swap.token_out)?;

        Some((
            (adjusted_out / adjusted_in),
            centralized_prices.0.clone(),
            centralized_prices.1.clone(),
        ))
    }
}

#[async_trait::async_trait]
impl Inspector for CexDexInspector {
    async fn process_tree(
        &self,
        tree: Arc<TimeTree<Actions>>,
        meta_data: Arc<Metadata>,
    ) -> Vec<(ClassifiedMev, Box<dyn SpecificMev>)> {
        let intersting_state =
            tree.inspect_all(|node| node.subactions.iter().any(|action| action.is_swap()));

        intersting_state
            .into_par_iter()
            .filter_map(|(tx, nested_swaps)| {
                let gas_details = tree.get_gas_details(tx)?;
                // Flatten the nested Vec<Vec<V>> into a Vec<V>
                let swaps = nested_swaps.into_iter().collect::<Vec<_>>();
                let root = tree.get_root(tx)?;
                let eoa = root.head.address;
                let mev_contract = root.head.data.get_too_address();
                self.process_swap(tx, mev_contract, eoa, meta_data.clone(), gas_details, swaps)
            })
            .collect::<Vec<_>>()
    }
}
