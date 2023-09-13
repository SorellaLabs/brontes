use std::sync::Arc;
use std::collections::HashMap;
use malachite::{num::conversion::traits::RoundingFrom, rounding_modes::RoundingMode, Rational};
use poirot_labeller::Metadata;
use poirot_types::{
    normalized_actions::{Actions, NormalizedSwap},
    TOKEN_TO_DECIMALS,
    tree::{GasDetails, TimeTree}, ToScaledRational,
};

use rayon::iter::{IntoParallelIterator, ParallelIterator};
use reth_primitives::{H256, Address, U256};
use tracing::error;

use crate::{ClassifiedMev, Inspector};

pub struct CexDexInspector;

impl CexDexInspector {
    fn process_swaps(
        &self,
        hash: H256,
        priority_fee: u64,
        metadata: Arc<Metadata>,
        gas_details: &GasDetails,
        swaps: Vec<Vec<Actions>>
    ) -> Option<ClassifiedMev> {
        let cex_dex_d = self.cex_dex_profit_no_gas(swaps, metadata);


        let appearance_usd_deltas = self.get_best_usd_delta(
            deltas.clone(),
            metadata.clone(),
            Box::new(|(appearance, _)| appearance)
        );

        let finalized_usd_deltas =
            self.get_best_usd_delta(deltas, metadata.clone(), Box::new(|(_, finalized)| finalized));

        if finalized_usd_deltas.is_none() || appearance_usd_deltas.is_none() {
            return None
        }
        let (finalized, appearance) =
            (finalized_usd_deltas.unwrap(), appearance_usd_deltas.unwrap());

        if finalized.0 != appearance.0 {
            error!("finalized addr != appearance addr");
            return None
        }

        let gas_used = gas_details.gas_paid();
        let (gas_used_usd_appearance, gas_used_usd_finalized) = (
            Rational::from(gas_used) * &metadata.eth_prices.0,
            Rational::from(gas_used) * &metadata.eth_prices.1
        );

        Some(ClassifiedMev {
            contract: finalized.0,
            gas_details: vec![gas_details.clone()],
            tx_hash: vec![hash],
            priority_fee: vec![priority_fee],
            block_finalized_profit_usd: f64::rounding_from(
                &finalized.1 - gas_used_usd_finalized,
                RoundingMode::Nearest
            )
            .0,
            block_appearance_profit_usd: f64::rounding_from(
                &appearance.1 - gas_used_usd_appearance,
                RoundingMode::Nearest
            )
            .0,
            block_finalized_revenue_usd: f64::rounding_from(finalized.1, RoundingMode::Nearest).0,
            block_appearance_revenue_usd: f64::rounding_from(appearance.1, RoundingMode::Nearest).0
        })
    }


    pub fn cex_dex_profit_no_gas(
        &self,
        swaps: Vec<Vec<Actions>>,
        metadata: Arc<Metadata>,
    ) -> HashMap<Address, HashMap<Actions, Rational>> {
        let mut deltas = HashMap::new();
    
        for actions in swaps.iter() {
            for action in actions.iter().map(|action| ) {
                if let Actions::Swap(swap) = action {
                    
                    // think it should be fine to unwrap here because it should never fail but we never know
                    
                    
    
                    
    
                    let (cex_price1, cex_price2) = centralized_prices.unwrap();
    
                    let dex_price = dex_price.unwrap();
                    if *cex_price1 > dex_price && *cex_price2 > dex_price {
                        //TODO: very terroristic, will finish tmrw should be chill 
                        
                        

                    }
                }
            }
        }
    
        deltas
    }

    pub fn get_cex_dex(&self, swap: &NormalizedSwap, metadata: &Metadata, dex_price: Rational) -> Option<(&Rational, &Rational)> {
        let arb = &self.rational_dex_price(&swap, metadata).and_then(| (dex_price, cex_price1, cex_price2 )| {
            self.profit_classifier(swap, dex_price, cex_price1, cex_price2)
        });
    }

    fn profit_classifier(&self,swap: &NormalizedSwap, dex_price: &Rational, cex_price1: &Rational, cex_price2: &Rational, ) -> Option<(Option<Rational>, Option<Rational>)> {
       // Calculate the price differences between DEX and CEX
        let delta_price1 = cex_price1 - dex_price;
        let delta_price2 = cex_price2 - dex_price;

        // Calculate the potential profit
        let profit1 = delta_price1 * swap.amount_in.to_scaled_rational(18);
        let profit2 = delta_price2 * swap.amount_in.to_scaled_rational(18);

        // Check if the profit is positive 
        match (profit1 > Rational::from(0), profit2 > Rational::from(0)) {
            (true, true) => Some((Some(profit1), Some(profit2))),
            (true, false) => Some((Some(profit1), None)),
            (false, true) => Some((None, Some(profit2))),
            (false, false) => None,
        }
    }


    pub fn rational_dex_price(
        &self,
        swap: &NormalizedSwap,
        metadata: &Metadata,
    ) -> Option<(&Rational, &Rational, &Rational)> {
        let Some(decimals_in) = TOKEN_TO_DECIMALS.get(&swap.token_in.0) else {
            error!(missing_token=?swap.token_in, "missing token in token to decimal map");
            return None
        };
        //TODO(JOE): this is ugly asf, but we should have some metrics shit so we can log it
        let Some(decimals_out) = TOKEN_TO_DECIMALS.get(&swap.token_out.0) else {
            error!(missing_token=?swap.token_in, "missing token in token to decimal map");
            return None
        };

        let adjusted_in = swap.amount_in.to_scaled_rational(*decimals_in);
        let adjusted_out = swap.amount_out.to_scaled_rational(*decimals_out);

        let centralized_prices = metadata.token_prices.get(&swap.token_out)?;

        Some((&(adjusted_out / adjusted_in), &centralized_prices.0, &centralized_prices.1))
    }
}

//TODO(WILL) I think we should reorganise the way we do priority fee, becuase why force a unecessary call on all inspectos when we can just
//TODO calc & store it when we build the tree, because all inspectors need it (same as bribe amount)
#[async_trait::async_trait]
impl Inspector for CexDexInspector {
    async fn process_tree(
        &self,
        tree: Arc<TimeTree<Actions>>,
        meta_data: Arc<Metadata>
    ) -> Vec<ClassifiedMev> {
        let intersting_state = tree.inspect_all(|node| {
            node.subactions
                .iter()
                .any(|action| action.is_swap())
        });

        intersting_state
            .into_par_iter()
            .filter_map(|(tx, swaps)| {
                let gas_details = tree.get_gas_details(tx)?;
                self.process_swaps(
                    tx,
                    tree.get_priority_fee_for_transaction(tx).unwrap(),
                    meta_data.clone(),
                    gas_details,
                    swaps
                )
            })
            .collect::<Vec<_>>()
    }
}

pub struct AtomicArb {}
