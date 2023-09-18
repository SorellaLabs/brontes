use std::{collections::HashMap, sync::Arc};

use malachite::{
    num::{basic::traits::Zero, conversion::traits::RoundingFrom},
    rounding_modes::RoundingMode,
    Rational
};
use poirot_classifer::enum_unwrap;
use poirot_labeller::Metadata;
use poirot_types::{
    classified_mev::SpecificMev,
    normalized_actions::{Actions, NormalizedSwap},
    tree::{GasDetails, TimeTree},
    ToScaledRational, TOKEN_TO_DECIMALS
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use reth_primitives::{Address, H256, U256};
use tracing::error;

use crate::{ClassifiedMev, Inspector};

pub struct CexDexInspector;

impl CexDexInspector {
    fn process_swap(
        &self,
        hash: H256,
        metadata: Arc<Metadata>,
        gas_details: &GasDetails,
        swaps: Vec<Actions>
    ) -> Vec<(ClassifiedMev, Box<dyn SpecificMev>)> {
        let (swap_data, (pre, post)) = swaps
            .into_iter()
            .filter_map(|action| {
                if let Actions::Swap(normalized_swap) = action {
                    let (pre, post) = self.get_cex_dex(&normalized_swap, metadata.as_ref());
                    Some((normalized_swap, (pre, post)))
                } else {
                    None
                }
            })
            .unzip();

        let profit_pre = self.arb_gas(pre, gas_details, metadata.eth_prices.0);
        let profit_post = self.arb_gas(post, gas_details, metadata.eth_prices.1);

        if profit_pre.is_some() || profit_post.is_some() {
            let mev = Some(ClassifiedMev {
                tx_hash:      vec![hash],
                block_number: metadata.block_num,
                mev_bot:      swap[0].call_address
            });
            return mev
        }
        None
    }

    fn arb_gas(
        &self,
        arbs: Vec<Option<Rational>>,
        gas_details: &GasDetails,
        eth_price: Rational
    ) -> Option<Rational> {
        Some(
            arbs.into_iter().flatten().sum::<Rational>()
                - Rational::from(gas_details.gas_paid()) * eth_price
        )
        .filter(|&p| p > Rational::ZERO)
    }

    pub fn get_cex_dex(
        &self,
        swap: &NormalizedSwap,
        metadata: &Metadata
    ) -> (Option<Rational>, Option<Rational>) {
        self.rational_dex_price(&swap, metadata)
            .map(|(dex_price, cex_price1, cex_price2)| {
                let profit1 = self.profit_classifier(swap, &dex_price, &cex_price1);
                let profit2 = self.profit_classifier(swap, &dex_price, &cex_price2);

                (
                    Some(profit1).filter(|p| Rational::ZERO.lt(p)),
                    Some(profit2).filter(|p| Rational::ZERO.lt(p))
                )
            })
            .unwrap_or((None, None))
    }

    fn profit_classifier(
        &self,
        swap: &NormalizedSwap,
        dex_price: &Rational,
        cex_price: &Rational
    ) -> Rational {
        // Calculate the price differences between DEX and CEX
        let delta_price = cex_price - dex_price;

        // Calculate the potential profit
        delta_price * swap.amount_in.to_scaled_rational(18)
    }

    pub fn rational_dex_price(
        &self,
        swap: &NormalizedSwap,
        metadata: &Metadata
    ) -> Option<(Rational, Rational, Rational)> {
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

        Some(((adjusted_out / adjusted_in), centralized_prices.0, centralized_prices.1))
    }
}

#[async_trait::async_trait]
impl Inspector for CexDexInspector {
    async fn process_tree(
        &self,
        tree: Arc<TimeTree<Actions>>,
        meta_data: Arc<Metadata>
    ) -> Vec<ClassifiedMev> {
        let intersting_state =
            tree.inspect_all(|node| node.subactions.iter().any(|action| action.is_swap()));

        intersting_state
            .into_par_iter()
            .filter_map(|(tx, nested_swaps)| {
                let gas_details = tree.get_gas_details(tx)?;
                // Flatten the nested Vec<Vec<V>> into a Vec<V>
                let swaps = nested_swaps.into_iter().flatten().collect::<Vec<_>>();
                self.process_swap(tx, meta_data.clone(), gas_details, swaps)
            })
            .collect::<Vec<_>>()
    }
}
