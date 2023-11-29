use std::{str::FromStr, sync::Arc};

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
        // the collectors for now. Will need to

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
            .filter_map(|swap| self.rational_prices(swap, &metadata))
            .map(|(dex_price, _, cex1)| (dex_price.to_float(), cex1.to_float()))
            .collect::<Vec<_>>();

        let flat_swaps = swaps.into_iter().flatten().collect::<Vec<_>>();

        let cex_dex = CexDex {
            tx_hash:          hash,
            gas_details:      gas_details.clone(),
            swaps_index:      flat_swaps
                .iter()
                .filter(|s| s.is_swap())
                .map(|s| s.clone().force_swap().index)
                .collect::<Vec<_>>(),
            swaps_from:       flat_swaps
                .iter()
                .filter(|s| s.is_swap())
                .map(|s| s.clone().force_swap().from)
                .collect::<Vec<_>>(),
            swaps_pool:       flat_swaps
                .iter()
                .filter(|s| s.is_swap())
                .map(|s| s.clone().force_swap().pool)
                .collect::<Vec<_>>(),
            swaps_token_in:   flat_swaps
                .iter()
                .filter(|s| s.is_swap())
                .map(|s| s.clone().force_swap().token_in)
                .collect::<Vec<_>>(),
            swaps_token_out:  flat_swaps
                .iter()
                .filter(|s| s.is_swap())
                .map(|s| s.clone().force_swap().token_out)
                .collect::<Vec<_>>(),
            swaps_amount_in:  flat_swaps
                .iter()
                .filter(|s| s.is_swap())
                .map(|s| s.clone().force_swap().amount_in.to())
                .collect::<Vec<_>>(),
            swaps_amount_out: flat_swaps
                .iter()
                .filter(|s| s.is_swap())
                .map(|s| s.clone().force_swap().amount_out.to())
                .collect::<Vec<_>>(),
            prices_kind:      prices
                .iter()
                .flat_map(|_| vec![PriceKind::Dex, PriceKind::Cex])
                .collect(),
            prices_address:   flat_swaps
                .iter()
                .filter(|s| s.is_swap())
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
        let zero = Rational::ZERO;
        let (total_pre_arb, total_post_arb) = swap_sequences
            .iter()
            .flat_map(|sequence| sequence)
            .fold((Rational::ZERO, Rational::ZERO), |(acc_pre, acc_post), (_, (pre, post))| {
                (acc_pre + pre.as_ref().unwrap_or(&zero), acc_post + post.as_ref().unwrap_or(&zero))
            });

        let gas_cost_pre =
            Rational::from_unsigneds(gas_details.gas_paid(), 10u64.pow(18)) * eth_price_pre;
        let gas_cost_post =
            Rational::from_unsigneds(gas_details.gas_paid(), 10u64.pow(18)) * eth_price_post;

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
        self.rational_prices(&Actions::Swap(swap.clone()), metadata)
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

    pub fn rational_prices(
        &self,
        swap: &Actions,
        metadata: &Metadata,
    ) -> Option<(Rational, Rational, Rational)> {
        let Actions::Swap(swap) = swap else { return None };

        let Some(decimals_in) = TOKEN_TO_DECIMALS.get(&swap.token_in.0) else {
            error!(missing_token=?swap.token_in, "missing token in token to decimal map");
            println!("missing token in token to decimal map");
            return None
        };
        //TODO(JOE): this is ugly asf, but we should have some metrics shit so we can
        // log it
        let Some(decimals_out) = TOKEN_TO_DECIMALS.get(&swap.token_out.0) else {
            error!(missing_token=?swap.token_out, "missing token out token to decimal map");
            println!("missing token in token to decimal map");
            return None
        };

        let adjusted_in = swap.amount_in.to_scaled_rational(*decimals_in);
        let adjusted_out = swap.amount_out.to_scaled_rational(*decimals_out);

        let token_out_centralized_price = metadata.token_prices.get(&swap.token_out)?;
        let token_in_centralized_price = metadata.token_prices.get(&swap.token_in)?;

        Some((
            (adjusted_out / adjusted_in),
            &token_out_centralized_price.0 / &token_in_centralized_price.0,
            &token_out_centralized_price.1 / &token_in_centralized_price.1,
        ))
    }
}

#[cfg(test)]
mod tests {

    use std::{
        collections::{HashMap, HashSet},
        env,
        str::FromStr,
        time::SystemTime,
    };

    use brontes_classifier::Classifier;
    use brontes_core::test_utils::{init_trace_parser, init_tracing};
    use brontes_database::database::Database;
    use brontes_types::test_utils::write_tree_as_json;
    use malachite::num::{basic::traits::One, conversion::traits::FromSciString};
    use reth_primitives::U256;
    use serial_test::serial;
    use tokio::sync::mpsc::unbounded_channel;
    use tracing::info;

    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_cex_dex() {
        init_tracing();

        info!(target: "brontes", "we got it");
        dotenv::dotenv().ok();
        let block_num = 18264694;

        let (tx, _rx) = unbounded_channel();

        let tracer = init_trace_parser(tokio::runtime::Handle::current().clone(), tx);
        let db = Database::default();
        let classifier = Classifier::new();
        println!("{:?}", db.credentials());

        let block = tracer.execute_block(block_num).await.unwrap();
        let metadata = db.get_metadata(block_num).await;

        println!("{:#?}", metadata);

        let tx = block.0.clone().into_iter().take(40).collect::<Vec<_>>();
        let tree = Arc::new(classifier.build_tree(tx, block.1, &metadata));

        //write_tree_as_json(&tree, "./tree.json").await;

        let inspector = CexDexInspector::default();

        let t0 = SystemTime::now();
        let mev = inspector.process_tree(tree.clone(), metadata.into()).await;
        let t1 = SystemTime::now();
        let delta = t1.duration_since(t0).unwrap().as_micros();
        println!("{:#?}", mev);

        println!("cex-dex inspector took: {} us", delta);

        // assert!(
        //     mev[0].0.tx_hash
        //         == H256::from_str(
        //
        // "0x80b53e5e9daa6030d024d70a5be237b4b3d5e05d30fdc7330b62c53a5d3537de"
        //         )
        //         .unwrap()
        // );
    }

    //Testing for tx:
    // 0x21b129d221a4f169de0fc391fe0382dbde797b69300a9a68143487c54d620295

    #[tokio::test]
    #[serial]
    async fn test_profit_calculation() {
        init_tracing();
        let block_num = 18264694;

        let swap = NormalizedSwap {
            index:      0,
            from:       Address::from_str("0xA69babEF1cA67A37Ffaf7a485DfFF3382056e78C").unwrap(),
            pool:       Address::from_str("0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640").unwrap(),
            token_in:   Address::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap(),
            token_out:  Address::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap(),
            amount_in:  "5055369263870573349743".parse().unwrap(),
            amount_out: "8421308582396".parse().unwrap(),
        };

        // ETH Sold = 5,055.369263
        // USDC bought = 8 421 308.582396
        // price = $1665.8147297
        // See Chart: https://www.tradingview.com/x/eLfjxI9h
        //
        // We need to integrate more granular data because otherwise I think the binance
        // quotes are out of whack at that time TBD

        let metadata = Metadata {
            block_num:              18264694,
            block_hash:             U256::from_str_radix(
                "57968198764731c3fcdb0caff812559ce5035aabade9e6bcb2d7fcee29616729",
                16,
            )
            .unwrap(),
            relay_timestamp:        1696271963129, // Oct 02 2023 18:39:23 UTC
            p2p_timestamp:          1696271964134, // Oct 02 2023 18:39:24 UTC
            proposer_fee_recipient: Address::from_str("0x388c818ca8b9251b393131c08a736a67ccb19297")
                .unwrap(),
            proposer_mev_reward:    11769128921907366414,
            token_prices:           {
                let mut prices = HashMap::new();

                // By looking at the chart, and comparing it to the binance quote we can see
                // that our quotes are lagging:
                // - 1: If we can get a chart that shows us 1s time frames we can tell if quotes
                //   are out of whack but I doubt this is the problem
                // - 2: Most likely that the quotes are correct, their signals are forward
                //   looking by definition so we need to get CEX quotes at tx time + time frame.

                // At 18:39:23 UTC (time of submission) the price is $1682.268937
                // At 18:40 UTC (lowest level granularity I could get from the ) the price is
                // $1682.081816

                // See chart: https://www.tradingview.com/x/5uG0Zxdq
                prices.insert(
                    Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
                    (
                        Rational::from_str("7398697029111485/4398046511104").unwrap(),  // WETH = $1682.268937
                        Rational::from_str("924734257781285/549755813888").unwrap(), // WETH = $1682.081816
                    ),
                );

                // USDC = $1
                prices.insert(
                    Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),// USDC 
                    (
                        Rational::from_str("1").unwrap(), // Assuming 1 USDC = 1 USD for simplicity, replace with actual values
                        Rational::from_str("1").unwrap(),
                    ),
                );
                prices
            },
            eth_prices:             (
                Rational::from_str("7398697029111485/4398046511104").unwrap(),
                Rational::from_str("924734257781285/549755813888").unwrap(),
            ),
            mempool_flow:           {
                let mut private = HashSet::new();
                private.insert(
                    H256::from_str(
                        "0x21b129d221a4f169de0fc391fe0382dbde797b69300a9a68143487c54d620295",
                    )
                    .unwrap(),
                );
                private
            },
        };

        let (tx, _rx) = unbounded_channel();

        let tracer = init_trace_parser(tokio::runtime::Handle::current().clone(), tx);
        let db = Database::default();
        let classifier = Classifier::new();

        let inspector = CexDexInspector::default();
        let profit = inspector.get_cex_dex(&swap, &metadata);

        //assert_eq!(profit, (Some(Rational::from) None));
    }

    #[tokio::test]
    #[serial]
    async fn test_rational_price() {
        init_tracing();
        let swap = NormalizedSwap {
            index:      0,
            from:       Address::from_str("0xA69babEF1cA67A37Ffaf7a485DfFF3382056e78C").unwrap(),
            pool:       Address::from_str("0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640").unwrap(),
            token_in:   Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap(),
            token_out:  Address::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap(),
            amount_in:  U256::from_str("5055369263000000000000").unwrap(),
            amount_out: U256::from_str("8421308582396").unwrap(),
        };

        // ETH Sold = 5,055.369263
        // USDC bought = 8 421 308.582396
        // price = $1665.8147297

        let metadata = Metadata {
            block_num:              18264694,
            block_hash:             U256::from_str_radix(
                "57968198764731c3fcdb0caff812559ce5035aabade9e6bcb2d7fcee29616729",
                16,
            )
            .unwrap(),
            relay_timestamp:        1696271963129, // Oct 02 2023 18:39:23 UTC
            p2p_timestamp:          1696271964134, // Oct 02 2023 18:39:24 UTC
            proposer_fee_recipient: Address::from_str("0x388c818ca8b9251b393131c08a736a67ccb19297")
                .unwrap(),
            proposer_mev_reward:    11769128921907366414,
            token_prices:           {
                let mut prices = HashMap::new();

                // WETH = $1682.268937
                prices.insert(
                    Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
                    (
                        Rational::from_str("7398697029111485/4398046511104").unwrap(),
                        Rational::from_str("924734257781285/549755813888").unwrap(),
                    ),
                );

                // USDC = $1
                prices.insert(
                    Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),// USDC 
                    (
                        Rational::from_str("1").unwrap(), // Assuming 1 USDC = 1 USD for simplicity, replace with actual values
                        Rational::from_str("1").unwrap(),
                    ),
                );
                prices
            },
            eth_prices:             (
                Rational::from_str("7398697029111485/4398046511104").unwrap(),
                Rational::from_str("924734257781285/549755813888").unwrap(),
            ),
            mempool_flow:           {
                let mut private = HashSet::new();
                private.insert(
                    H256::from_str(
                        "0x21b129d221a4f169de0fc391fe0382dbde797b69300a9a68143487c54d620295",
                    )
                    .unwrap(),
                );
                private
            },
        };

        let inspector = CexDexInspector::default();
        let rational_prices = inspector.rational_prices(&Actions::Swap(swap.clone()), &metadata);

        let amount_in = Rational::from_sci_string("5055369263000000000000e-18").unwrap();
        let amount_out = Rational::from_sci_string("8421308582396e-6").unwrap();

        let expected_dex_price = amount_out / amount_in;

        assert_eq!(
            expected_dex_price,
            rational_prices
                .as_ref()
                .unwrap_or(&(Rational::ZERO, Rational::ZERO, Rational::ZERO))
                .0,
            "Dex price did not match"
        );

        let expected_cex_price1 = metadata
            .token_prices
            .get(&swap.token_out)
            .unwrap()
            .0
            .clone()
            / metadata.token_prices.get(&swap.token_in).unwrap().0.clone();

        assert_eq!(
            expected_cex_price1,
            rational_prices.as_ref().unwrap().1,
            "Pre cex price did not match {}",
            rational_prices.as_ref().unwrap().1
        );

        let expected_cex_price2 = metadata
            .token_prices
            .get(&swap.token_out)
            .unwrap()
            .1
            .clone()
            / metadata.token_prices.get(&swap.token_in).unwrap().1.clone();

        assert_eq!(
            expected_cex_price2,
            rational_prices.as_ref().unwrap().2,
            "Post cex price did not match {}",
            rational_prices.as_ref().unwrap().2
        );
    }

    #[tokio::test]
    async fn test_arb_gas_accounting() {
        init_tracing();
        let mut swaps = Vec::new();
        let gas_details = GasDetails {
            coinbase_transfer:   None,
            priority_fee:        0,
            gas_used:            20_000,
            // 20 gewi
            effective_gas_price: 20 * 10_u64.pow(9),
        };

        let swap = NormalizedSwap::default();

        let pre_0 = Rational::from(10);
        let post_0 = Rational::from(10);
        let swaped = Actions::Swap(swap.clone());
        let inner_0 = vec![(&swaped, (Some(pre_0), Some(post_0)))];
        swaps.push(inner_0);

        let inspector = CexDexInspector::default();
        let eth_price_pre = Rational::from(1);
        let eth_price_post = Rational::from(2);

        let (pre, post) =
            inspector.arb_gas_accounting(swaps, &gas_details, &eth_price_pre, &eth_price_post);

        let pre = pre.unwrap();
        let post = post.unwrap();
        let pre_result = Rational::from_str("24999/2500").unwrap();
        let post_result = Rational::from_str("12499/1250").unwrap();

        assert_eq!(pre, pre_result);
        assert_eq!(post, post_result);
    }
}
