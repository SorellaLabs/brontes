use std::sync::Arc;

use brontes_database::Metadata;
use brontes_types::{
    classified_mev::{CexDex, MevType, PriceKind, SpecificMev},
    normalized_actions::{Actions, NormalizedSwap},
    tree::{GasDetails, TimeTree},
    ToFloatNearest, ToScaledRational, TOKEN_TO_DECIMALS,
};
use malachite::{num::basic::traits::Zero, Rational};
use rayon::{
    iter::{IntoParallelIterator, ParallelIterator},
    prelude::IntoParallelRefIterator,
};
use reth_primitives::{Address, H256};
use tracing::error;

use crate::{shared_utils::SharedInspectorUtils, ClassifiedMev, Inspector};

#[derive(Default)]
pub struct CexDexInspector {
    inner: SharedInspectorUtils,
}

#[async_trait::async_trait]
impl Inspector for CexDexInspector {
    async fn process_tree(
        &self,
        tree: Arc<TimeTree<Actions>>,
        meta_data: Arc<Metadata>,
    ) -> Vec<(ClassifiedMev, Box<dyn SpecificMev>)> {
        // Get all normalized swaps
        let intersting_state =
            tree.inspect_all(|node| node.subactions.iter().any(|action| action.is_swap()));

        intersting_state
            .into_par_iter()
            .filter_map(|(tx, nested_swaps)| {
                let gas_details = tree.get_gas_details(tx)?;

                let root = tree.get_root(tx)?;
                let eoa = root.head.address;
                let mev_contract = root.head.data.get_to_address();
                self.process_swaps(
                    tx,
                    mev_contract,
                    eoa,
                    meta_data.clone(),
                    gas_details,
                    nested_swaps,
                )
            })
            .collect::<Vec<_>>()
    }
}

impl CexDexInspector {
    fn process_swaps(
        &self,
        hash: H256,
        mev_contract: Address,
        eoa: Address,
        metadata: Arc<Metadata>,
        gas_details: &GasDetails,
        swaps: Vec<Vec<Actions>>,
    ) -> Option<(ClassifiedMev, Box<dyn SpecificMev>)> {
        let swap_sequences: Vec<Vec<(&Actions, (_, _))>> = swaps
            .iter()
            .map(|swap_sequence| {
                swap_sequence
                    .into_iter()
                    .filter_map(|action| {
                        if let Actions::Swap(ref normalized_swap) = action {
                            let (pre, post) = self.get_cex_dex(normalized_swap, metadata.as_ref());
                            Some((action, (pre, post)))
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .collect();

        let (profit_sub, profit_finalized) = self.arb_gas_accounting(
            swap_sequences,
            gas_details,
            &metadata.eth_prices.0,
            &metadata.eth_prices.1,
        );

        let (gas_sub, gas_finalized) = metadata.get_gas_price_usd(gas_details.gas_paid());

        // TODO: feels unecessary to do this again, given we have already looped through
        // the swaps in a less generic way, but this is the lowest effort way of getting
        // the collectors for now

        let deltas = self.inner.calculate_swap_deltas(&swaps);
        let mev_profit_collector = self
            .inner
            .get_best_usd_deltas(
                deltas.clone(),
                metadata.clone(),
                Box::new(|(appearance, _)| appearance),
            )
            .keys()
            .copied()
            .collect();

        let classified = ClassifiedMev {
            mev_profit_collector,
            tx_hash: hash,
            mev_contract,
            eoa,
            block_number: metadata.block_num,
            mev_type: MevType::CexDex,
            submission_profit_usd: profit_sub?.to_float(),
            finalized_profit_usd: profit_finalized?.to_float(),
            submission_bribe_usd: gas_sub.to_float(),
            finalized_bribe_usd: gas_finalized.to_float(),
        };

        let prices = swaps
            .par_iter()
            .flatten()
            .filter_map(|swap| self.rational_dex_price(swap, &metadata))
            .map(|(dex_price, _, cex1)| (dex_price.to_float(), cex1.to_float()))
            .collect::<Vec<_>>();

        let flat_swaps = swaps.into_iter().flatten().collect::<Vec<_>>();

        let cex_dex = CexDex {
            tx_hash:          hash,
            gas_details:      gas_details.clone(),
            swaps_index:      flat_swaps
                .iter()
                .map(|s| s.clone().force_swap().index)
                .collect::<Vec<_>>(),
            swaps_from:       flat_swaps
                .iter()
                .map(|s| s.clone().force_swap().from)
                .collect::<Vec<_>>(),
            swaps_pool:       flat_swaps
                .iter()
                .map(|s| s.clone().force_swap().pool)
                .collect::<Vec<_>>(),
            swaps_token_in:   flat_swaps
                .iter()
                .map(|s| s.clone().force_swap().token_in)
                .collect::<Vec<_>>(),
            swaps_token_out:  flat_swaps
                .iter()
                .map(|s| s.clone().force_swap().token_out)
                .collect::<Vec<_>>(),
            swaps_amount_in:  flat_swaps
                .iter()
                .map(|s| s.clone().force_swap().amount_in.to())
                .collect::<Vec<_>>(),
            swaps_amount_out: flat_swaps
                .iter()
                .map(|s| s.clone().force_swap().amount_out.to())
                .collect::<Vec<_>>(),
            prices_kind:      prices
                .iter()
                .flat_map(|_| vec![PriceKind::Dex, PriceKind::Cex])
                .collect(),
            prices_address:   flat_swaps
                .iter()
                .flat_map(|s| vec![s.clone().force_swap().token_in].repeat(2))
                .collect(),
            prices_price:     prices
                .iter()
                .flat_map(|(dex, cex)| vec![*dex, *cex])
                .collect(),
        };

        Some((classified, Box::new(cex_dex)))
    }

    fn arb_gas_accounting(
        &self,
        swap_sequences: Vec<Vec<(&Actions, (Option<Rational>, Option<Rational>))>>,
        gas_details: &GasDetails,
        eth_price_pre: &Rational,
        eth_price_post: &Rational,
    ) -> (Option<Rational>, Option<Rational>) {
        let (total_pre_arb, total_post_arb) = swap_sequences
            .iter()
            .flat_map(|sequence| sequence.iter())
            .fold((Rational::ZERO, Rational::ZERO), |(acc_pre, acc_post), (_, (pre, post))| {
                (
                    acc_pre + pre.clone().unwrap_or(Rational::ZERO),
                    acc_post + post.clone().unwrap_or(Rational::ZERO),
                )
            });

        let gas_cost_pre = Rational::from(gas_details.gas_paid()) * eth_price_pre;
        let gas_cost_post = Rational::from(gas_details.gas_paid()) * eth_price_post;

        let profit_pre =
            if total_pre_arb > gas_cost_pre { Some(total_pre_arb - gas_cost_pre) } else { None };

        let profit_post = if total_post_arb > gas_cost_post {
            Some(total_post_arb - gas_cost_post)
        } else {
            None
        };

        (profit_pre, profit_post)
    }

    // TODO check correctness + check cleanup potential with shared utils?
    pub fn get_cex_dex(
        &self,
        swap: &NormalizedSwap,
        metadata: &Metadata,
    ) -> (Option<Rational>, Option<Rational>) {
        self.rational_dex_price(&Actions::Swap(swap.clone()), metadata)
            .map(|(dex_price, cex_price1, cex_price2)| {
                let profit1 = self.profit_classifier(swap, &dex_price, &cex_price1);
                let profit2 = self.profit_classifier(swap, &dex_price, &cex_price2);

                (profit1.filter(|p| Rational::ZERO.lt(p)), profit2.filter(|p| Rational::ZERO.lt(p)))
            })
            .unwrap_or((None, None))
    }

    fn profit_classifier(
        &self,
        swap: &NormalizedSwap,
        dex_price: &Rational,
        cex_price: &Rational,
    ) -> Option<Rational> {
        // Calculate the price differences between DEX and CEX
        let delta_price = cex_price - dex_price;

        // Calculate the potential profit
        let Some(decimals_in) = TOKEN_TO_DECIMALS.get(&swap.token_in.0) else {
            error!(missing_token=?swap.token_in, "missing token in token to decimal map");
            println!("missing token in token to decimal map");
            return None
        };

        println!(
            "delta price: {}",
            &delta_price * &swap.amount_in.to_scaled_rational(*decimals_in)
        );
        Some(delta_price * swap.amount_in.to_scaled_rational(*decimals_in))
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

        let centralized_prices_out = metadata.token_prices.get(&swap.token_out)?;
        let centralized_prices_in = metadata.token_prices.get(&swap.token_in)?;

        Some((
            (adjusted_out / adjusted_in),
            &centralized_prices_out.0 / &centralized_prices_in.0,
            &centralized_prices_out.1 / &centralized_prices_in.1,
        ))
    }
}

#[cfg(test)]
mod tests {

    use std::{str::FromStr, time::SystemTime};

    use brontes_classifier::Classifier;
    use brontes_core::test_utils::init_trace_parser;
    use brontes_database::database::Database;
    use brontes_types::test_utils::write_tree_as_json;
    use serial_test::serial;
    use tokio::sync::mpsc::unbounded_channel;

    use super::*;
    #[tokio::test]
    #[serial]
    async fn test_cex_dex() {
        dotenv::dotenv().ok();
        let block_num = 17195495;

        let (tx, _rx) = unbounded_channel();

        let tracer = init_trace_parser(tokio::runtime::Handle::current().clone(), tx);
        let db = Database::default();
        let classifier = Classifier::new();

        let block = tracer.execute_block(block_num).await.unwrap();
        let metadata = db.get_metadata(block_num).await;

        println!("Token Prices:");
        for (address, (price_pre, price_post)) in &metadata.token_prices {
            println!(
                "Address: {:?}, Pre-Update Price: {}, Post-Update Price: {}",
                address, price_pre, price_post
            );
        }
        println!("{:#?}", metadata);

        let tx = block.0.clone().into_iter().take(40).collect::<Vec<_>>();
        let tree = Arc::new(classifier.build_tree(tx, block.1, &metadata));

        //write_tree_as_json(&tree, "./tree.json").await;

        let inspector = CexDexInspector::default();

        let t0 = SystemTime::now();
        let mev = inspector
            .process_tree(tree.clone(), metadata.clone().into())
            .await;
        let t1 = SystemTime::now();
        let delta = t1.duration_since(t0).unwrap().as_micros();
        println!("cex-dex inspector took: {} us", delta);
        println!("{:#?}", metadata);

        // assert!(
        //     mev[0].0.tx_hash
        //         == H256::from_str(
        //
        // "0x80b53e5e9daa6030d024d70a5be237b4b3d5e05d30fdc7330b62c53a5d3537de"
        //         )
        //         .unwrap()
        // );

        println!("{:#?}", mev);
    }
}
