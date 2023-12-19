use std::{
    any::Any,
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use async_scoped::{Scope, TokioScope};
use brontes_database::Metadata;
use brontes_types::{
    classified_mev::{compose_sandwich_jit, ClassifiedMev, MevBlock, MevType, SpecificMev},
    normalized_actions::Actions,
    tree::TimeTree,
    ToScaledRational,
};
use futures::FutureExt;
use lazy_static::lazy_static;
use malachite::{num::conversion::traits::RoundingFrom, rounding_modes::RoundingMode};
use reth_primitives::Address;
use tracing::{info, Subscriber};

use crate::Inspector;

type ComposeFunction = Option<
    Box<
        dyn Fn(
                Box<dyn Any + 'static>,
                Box<dyn Any + 'static>,
                ClassifiedMev,
                ClassifiedMev,
            ) -> (ClassifiedMev, Box<dyn SpecificMev>)
            + Send
            + Sync,
    >,
>;

/// we use this to define a filter that we can iterate over such that
/// everything is ordered properly and we have already composed lower level
/// actions that could effect the higher level composing.
macro_rules! mev_composability {
    ($($mev_type:ident => $($deps:ident),+;)+) => {
        lazy_static! {
        static ref MEV_FILTER: &'static [(
                MevType,
                ComposeFunction,
                Vec<MevType>)] = {
            &*Box::leak(Box::new([
                $((
                        MevType::$mev_type,
                        get_compose_fn(MevType::$mev_type),
                        [$(MevType::$deps,)+].to_vec()),
                   )+
            ]))
        };
    }
    };
}

mev_composability!(
    // reduce first
    Sandwich => Backrun, CexDex;
    // try compose
    JitSandwich => Sandwich, Jit;
);

/// the compose function is used in order to be able to properly be able to cast
/// in the lazy static
fn get_compose_fn(mev_type: MevType) -> ComposeFunction {
    match mev_type {
        MevType::JitSandwich => Some(Box::new(compose_sandwich_jit)),
        _ => None,
    }
}

// So for the master inspector we should get the address of the vertically
// integrated builders and know searcher addresses so we can also see when they
// are unprofitable and also better account for the profit given that they could
// be camouflaging thier trade by overbribing the builder given that
// they are one and the same

pub struct BlockPreprocessing {
    meta_data:           Arc<Metadata>,
    cumulative_gas_used: u128,
    cumulative_gas_paid: u128,
    builder_address:     Address,
}

type InspectorFut<'a> =
    Pin<Box<dyn Future<Output = Vec<(ClassifiedMev, Box<dyn SpecificMev>)>> + Send + 'a>>;

/// the results downcast using any in order to be able to serialize and
/// impliment row trait due to the abosulte autism that the db library   
/// requirements
pub type ComposerResults = (MevBlock, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>);

pub struct Composer<'a, const N: usize> {
    inspectors_execution: InspectorFut<'a>,
    pre_processing:       BlockPreprocessing,
}

impl<'a, const N: usize> Composer<'a, N> {
    pub fn new(
        orchestra: &'a [&'a Box<dyn Inspector>; N],
        tree: Arc<TimeTree<Actions>>,
        meta_data: Arc<Metadata>,
    ) -> Self {
        let processing = Self::pre_process(tree.clone(), meta_data.clone());
        let future = Box::pin(async move {
            let mut scope: TokioScope<'a, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>> =
                unsafe { Scope::create() };
            println!("inspectors to run: {}", orchestra.len());
            orchestra.iter().for_each(|inspector| {
                scope.spawn(inspector.process_tree(tree.clone(), meta_data.clone()))
            });

            scope
                .collect()
                .await
                .into_iter()
                .flat_map(|r| r.unwrap())
                .collect::<Vec<_>>()
        })
            as Pin<Box<dyn Future<Output = Vec<(ClassifiedMev, Box<dyn SpecificMev>)>> + 'a>>;

        Self {
            inspectors_execution: unsafe { std::mem::transmute(future) },
            pre_processing:       processing,
        }
    }

    // pub fn on_new_tree(&mut self, tree: Arc<TimeTree<Actions>>, meta_data:
    // Arc<Metadata>) {     // This is only unsafe due to the fact that you can
    // have missbehaviour where you     // drop this with incomplete futures
    //     let mut scope: TokioScope<'_, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>>
    // =         unsafe { Scope::create() };
    //
    //     println!("inspectors to run: {}", self.orchestra.len());
    //     self.orchestra.iter().for_each(|inspector| {
    //         scope.spawn(inspector.process_tree(tree.clone(), meta_data.clone()))
    //     });
    //
    //     let fut = Box::pin(async move {
    //         scope
    //             .collect()
    //             .map(|r| r.into_iter().flatten().flatten().collect::<Vec<_>>())
    //             .await
    //     });
    //
    //     self.pre_process(tree, meta_data);
    // }

    fn pre_process(tree: Arc<TimeTree<Actions>>, meta_data: Arc<Metadata>) -> BlockPreprocessing {
        let builder_address = tree.header.beneficiary;
        let cumulative_gas_used = tree
            .roots
            .iter()
            .map(|root| root.gas_details.gas_used)
            .sum::<u128>();

        let cumulative_gas_paid = tree
            .roots
            .iter()
            .map(|root| root.gas_details.effective_gas_price * root.gas_details.gas_used)
            .sum::<u128>();

        BlockPreprocessing { meta_data, cumulative_gas_used, cumulative_gas_paid, builder_address }
    }

    fn build_mev_header(
        &mut self,
        orchestra_data: &Vec<(ClassifiedMev, Box<dyn SpecificMev>)>,
    ) -> MevBlock {
        let pre_processing = &self.pre_processing;
        let cum_mev_priority_fee_paid = orchestra_data
            .iter()
            .map(|(_, mev)| mev.priority_fee_paid())
            .sum::<u128>();

        let total_bribe = 0;
        //TODO: need to substract proposer payement + fees paid for gas
        let builder_eth_profit = total_bribe + pre_processing.cumulative_gas_paid as i128;

        MevBlock {
            block_hash: pre_processing.meta_data.block_hash.into(),
            block_number: pre_processing.meta_data.block_num,
            mev_count: orchestra_data.len() as u64,
            finalized_eth_price: f64::rounding_from(
                &pre_processing.meta_data.eth_prices,
                RoundingMode::Nearest,
            )
            .0,
            cumulative_gas_used: pre_processing.cumulative_gas_used,
            cumulative_gas_paid: pre_processing.cumulative_gas_paid,
            total_bribe: orchestra_data
                .iter()
                .map(|(_, mev)| mev.bribe())
                .sum::<u128>(),
            cumulative_mev_priority_fee_paid: cum_mev_priority_fee_paid,
            builder_address: pre_processing.builder_address,
            builder_eth_profit,
            builder_finalized_profit_usd: f64::rounding_from(
                builder_eth_profit.to_scaled_rational(18) * &pre_processing.meta_data.eth_prices,
                RoundingMode::Nearest,
            )
            .0,
            proposer_fee_recipient: pre_processing.meta_data.proposer_fee_recipient,
            proposer_mev_reward: pre_processing.meta_data.proposer_mev_reward,
            proposer_finalized_profit_usd: f64::rounding_from(
                pre_processing
                    .meta_data
                    .proposer_mev_reward
                    .to_scaled_rational(18)
                    * &pre_processing.meta_data.eth_prices,
                RoundingMode::Nearest,
            )
            .0,
            cumulative_mev_finalized_profit_usd: f64::rounding_from(
                cum_mev_priority_fee_paid.to_scaled_rational(18)
                    * &pre_processing.meta_data.eth_prices,
                RoundingMode::Nearest,
            )
            .0,
        }
    }

    fn on_orchestra_resolution(
        &mut self,
        orchestra_data: Vec<(ClassifiedMev, Box<dyn SpecificMev>)>,
    ) -> Poll<ComposerResults> {
        info!("starting to compose classified mev");
        let header = self.build_mev_header(&orchestra_data);

        let mut sorted_mev = orchestra_data
            .into_iter()
            .map(|(classified_mev, specific)| (classified_mev.mev_type, (classified_mev, specific)))
            .fold(
                HashMap::default(),
                |mut acc: HashMap<MevType, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>>,
                 (mev_type, v)| {
                    acc.entry(mev_type).or_default().push(v);
                    acc
                },
            );

        MEV_FILTER
            .iter()
            .for_each(|(head_mev_type, compose_fn, dependencies)| {
                if let Some(compose_fn) = compose_fn {
                    self.compose_dep_filter(
                        head_mev_type,
                        dependencies,
                        compose_fn,
                        &mut sorted_mev,
                    );
                } else {
                    self.replace_dep_filter(head_mev_type, dependencies, &mut sorted_mev);
                }
            });

        // downcast all of the sorted mev results. should cleanup
        Poll::Ready((header, sorted_mev.into_values().flatten().collect::<Vec<_>>()))
    }

    fn replace_dep_filter(
        &mut self,
        head_mev_type: &MevType,
        deps: &[MevType],
        sorted_mev: &mut HashMap<MevType, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>>,
    ) {
        // TODO
        let Some(head_mev) = sorted_mev.get(head_mev_type) else { return };
        let flattend_indexes = head_mev
            .iter()
            .flat_map(|(_, specific)| {
                let hashes = specific.mev_transaction_hashes();
                let mut remove_data: Vec<(MevType, usize)> = Vec::new();
                for dep in deps {
                    let mut remove_count = 0;
                    let Some(dep_mev) = sorted_mev.get(dep) else { continue };

                    for (i, (_, specific)) in dep_mev.iter().enumerate() {
                        let dep_hashes = specific.mev_transaction_hashes();
                        // verify both match
                        if dep_hashes == hashes {
                            remove_data.push((*dep, i - remove_count));
                            remove_count += 1;
                            continue
                        }
                        // we only want one match
                        else if dep_hashes
                            .iter()
                            .map(|hash| hashes.contains(hash))
                            .any(|f| f)
                        {
                            remove_data.push((*dep, i - remove_count));
                            remove_count += 1;
                        }
                    }
                }

                remove_data
            })
            .collect::<Vec<(MevType, usize)>>();

        for (mev_type, index) in flattend_indexes {
            let entry = sorted_mev.get_mut(&mev_type).unwrap();
            if entry.len() > index {
                entry.remove(index);
            }
        }
    }

    fn compose_dep_filter(
        &mut self,
        parent_mev_type: &MevType,
        composable_types: &[MevType],
        compose: &Box<
            dyn Fn(
                    Box<dyn Any>,
                    Box<dyn Any>,
                    ClassifiedMev,
                    ClassifiedMev,
                ) -> (ClassifiedMev, Box<dyn SpecificMev>)
                + Send
                + Sync,
        >,
        sorted_mev: &mut HashMap<MevType, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>>,
    ) {
        if composable_types.len() != 2 {
            panic!("we only support sequential compatibility for our specific mev");
        }

        let Some(zero_txes) = sorted_mev.remove(&composable_types[0]) else { return };

        for (classified, mev_data) in zero_txes {
            let addresses = mev_data.mev_transaction_hashes();

            if let Some((index, _)) = sorted_mev.get(&composable_types[1]).and_then(|mev_type| {
                mev_type.iter().enumerate().find(|(_, (_, v))| {
                    let o_addrs = v.mev_transaction_hashes();
                    o_addrs == addresses || addresses.iter().any(|a| o_addrs.contains(a))
                })
            }) {
                // remove composed type
                let (classifed_1, mev_data_1) = sorted_mev
                    .get_mut(&composable_types[1])
                    .unwrap()
                    .remove(index);
                // insert new type
                sorted_mev
                    .entry(*parent_mev_type)
                    .or_default()
                    .push(compose(
                        mev_data.into_any(),
                        mev_data_1.into_any(),
                        classified,
                        classifed_1,
                    ));
            } else {
                // if no prev match, then add back old type
                sorted_mev
                    .entry(composable_types[0])
                    .or_default()
                    .push((classified, mev_data));
            }
        }
    }
}

impl<const N: usize> Future for Composer<'_, N> {
    type Output = ComposerResults;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Poll::Ready(calculation) = self.inspectors_execution.poll_unpin(cx) {
            return self.on_orchestra_resolution(calculation)
        }
        Poll::Pending
    }
}
/*q
#[cfg(test)]
pub mod tests {
    use std::{
        collections::{HashMap, HashSet},
        str::FromStr,
    };

    use brontes_classifier::Classifier;
    use brontes_core::test_utils::{init_trace_parser, init_tracing};
    use brontes_database::database::Database;
    use malachite::Rational;
    use reth_primitives::{B256, U256};
    use serial_test::serial;
    use tokio::sync::mpsc::unbounded_channel;
    use tracing::info;

    use super::*;
    use crate::{
        atomic_backrun::AtomicBackrunInspector, cex_dex::CexDexInspector, jit::JitInspector,
        sandwich::SandwichInspector,
    };

    unsafe fn cast_lifetime<'f, 'a, I>(item: &'a I) -> &'f I {
        std::mem::transmute::<&'a I, &'f I>(item)
    }

    fn get_metadata() -> Metadata {
        // 2126.43
        Metadata {
            block_num:              18539312,
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
            cex_quotes:             {
                let mut prices = HashMap::new();

                prices.insert(
                    Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
                    (
                        Rational::try_from_float_simplest(2126.43).unwrap(),
                        Rational::try_from_float_simplest(2126.43).unwrap(),
                    ),
                );

                // SMT
                prices.insert(
                    Address::from_str("0xb17548c7b510427baac4e267bea62e800b247173").unwrap(),
                    (
                        Rational::try_from_float_simplest(0.09081931).unwrap(),
                        Rational::try_from_float_simplest(0.09081931).unwrap(),
                    ),
                );

                // APX
                prices.insert(
                    Address::from_str("0xed4e879087ebd0e8a77d66870012b5e0dffd0fa4").unwrap(),
                    (
                        Rational::try_from_float_simplest(0.00004047064).unwrap(),
                        Rational::try_from_float_simplest(0.00004047064).unwrap(),
                    ),
                );
                // FTT
                prices.insert(
                    Address::from_str("0x50d1c9771902476076ecfc8b2a83ad6b9355a4c9").unwrap(),
                    (
                        Rational::try_from_float_simplest(1.9358).unwrap(),
                        Rational::try_from_float_simplest(1.9358).unwrap(),
                    ),
                );

                prices
            },
            eth_prices:             (Rational::try_from_float_simplest(2126.43).unwrap()),
            mempool_flow:           {
                let mut private = HashSet::new();
                private.insert(
                    B256::from_str(
                        "0x21b129d221a4f169de0fc391fe0382dbde797b69300a9a68143487c54d620295",
                    )
                    .unwrap(),
                );
                private
            },
        }
    }


    /// takes the blocknumber, setups the tree and calls on_new_tree before
    /// returning the composer
    pub async fn setup(block_num: u64, custom_meta: Option<Metadata>) -> Composer<'static, 2> {
        init_tracing();
        dotenv::dotenv().ok();

        let (tx, _rx) = unbounded_channel();

        let tracer = init_trace_parser(tokio::runtime::Handle::current().clone(), tx);
        let db = Database::default();
        let classifier = Classifier::new();

        let block = tracer.execute_block(block_num).await.unwrap();
        let metadata =
            if let Some(meta) = custom_meta { meta } else { db.get_metadata(block_num).await };

        let (tokens_missing_decimals, tree) = classifier.build_tree(block.0, block.1);

        let USDC = Address::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap();

        // Here the quote address is USDC
        let cex_dex = Box::new(CexDexInspector::new(USDC)) as Box<dyn Inspector>;
        let backrun = Box::new(AtomicBackrunInspector::new(USDC)) as Box<dyn Inspector>;
        let jit = Box::new(JitInspector::new(USDC)) as Box<dyn Inspector>;
        let sandwich = Box::new(SandwichInspector::new(USDC)) as Box<dyn Inspector>;

        let inspectors: [&'static Box<dyn Inspector>; 2] = unsafe {
            [
                // cast_lifetime::<'static>(&cex_dex),
                // cast_lifetime::<'static>(&backrun),
                cast_lifetime::<'static>(&jit),
                cast_lifetime::<'static>(&sandwich),
            ]
        };

        let mut composer = Composer::new(Box::leak(Box::new(inspectors)));
        composer.on_new_tree(tree.into(), metadata.into());

        composer
    }

    #[tokio::test]
    #[serial_test::serial]
    pub async fn test_jit_sandwich_composition() {
        let mut composer = setup(18539312, Some(get_metadata())).await;
        let (mev_block, classified_mev) = composer.await;
        info!("{:#?}\n\n{:#?}", mev_block, classified_mev);
    }

    #[tokio::test]
    #[serial]
    async fn test_jit() {
        init_tracing();
        dotenv::dotenv().ok();
        // testing https://eigenphi.io/mev/ethereum/tx/0x96a1decbb3787fbe26de84e86d6c2392f7ab7b31fb33f685334d49db2624a424
        // This is a jit sandwich, however we are just trying to detect the jit portion
        let block_num = 18539312;
    }

    #[tokio::test]
    #[serial_test::serial]
    pub async fn test_sandwich_jit_compose() {}
}
*/
