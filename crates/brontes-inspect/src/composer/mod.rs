use std::{
    any::Any,
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

mod utils;
use async_scoped::{Scope, TokioScope};
use brontes_database::Metadata;
use brontes_types::{
    classified_mev::{compose_sandwich_jit, ClassifiedMev, MevBlock, MevType, SpecificMev},
    normalized_actions::Actions,
    tree::BlockTree,
};
use futures::FutureExt;
use lazy_static::lazy_static;
use tracing::info;
use utils::{
    build_mev_header, find_mev_with_matching_tx_hashes, pre_process, sort_mev_by_type,
    BlockPreprocessing,
};

use crate::Inspector;

type ComposeFunction = Box<
    dyn Fn(
            Vec<(ClassifiedMev, Box<dyn Any + Send + Sync>)>,
        ) -> (ClassifiedMev, Box<dyn SpecificMev>)
        + Send
        + Sync,
>;

//TODO: Improve docs after reading
/// This macro is used to define an `MEV_COMPOSABILITY_FILTER`
///
/// The macro takes pairs of MEV types and their dependencies. Each pair is
/// defined as `MevType => DependentMevTypes;`.
///
/// The macro creates a static reference, `MEV_FILTER`, to an array of tuples.
/// Each tuple contains:
/// - An `MevType` which is the type of MEV being processed.
/// - A `ComposeFunction` which is the function used to compose the MEV of the
///   given type.
/// - A `Vec<MevType>` which is a vector of MEV types that the current MEV type
///   depends on.

#[macro_export]
macro_rules! mev_composability {
    ($($mev_type:ident => $($deps:ident),+;)+) => {
        lazy_static! {
        static ref MEV_COMPOSABILITY_FILTER: &'static [(
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
    JitSandwich => Sandwich, Jit;
);
//TODO: (Ludwig): Support arbitrary amount of dominant => dependent
// deduplication relationships so we can support long tail inspection and dedup
#[macro_export]
macro_rules! mev_deduplication {
    ($($mev_type:ident => $($deps:ident),+;)+) => {
        lazy_static! {
        static ref MEV_DEDUPLICATION_FILTER: &'static [(
                MevType,
                Vec<MevType>)] = {
            &*Box::leak(Box::new([
                $((
                        MevType::$mev_type,
                        [$(MevType::$deps,)+].to_vec()),
                   )+
            ]))
        };
    }
    };
}

mev_deduplication!(
    Sandwich => Backrun;
    CexDex => Backrun;
    Sandwich => CexDex;
);

/// the compose function is used in order to be able to properly cast
/// in the lazy static
fn get_compose_fn(mev_type: MevType) -> ComposeFunction {
    match mev_type {
        MevType::JitSandwich => Box::new(compose_sandwich_jit),
        _ => unreachable!("This mev type does not have a compose function"),
    }
}

type InspectorFut<'a> =
    Pin<Box<dyn Future<Output = Vec<(ClassifiedMev, Box<dyn SpecificMev>)>> + Send + 'a>>;

/// the results downcast using any in order to be able to serialize and
/// implement row trait due to the absolute autism that the db library   
/// requirements
pub type ComposerResults = (MevBlock, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>);

pub struct Composer<'a, const N: usize> {
    inspectors_execution: InspectorFut<'a>,
    pre_processing:       BlockPreprocessing,
    metadata:             Arc<Metadata>,
}

impl<'a, const N: usize> Composer<'a, N> {
    pub fn new(
        orchestra: &'a [&'a Box<dyn Inspector>; N],
        tree: Arc<BlockTree<Actions>>,
        meta_data: Arc<Metadata>,
    ) -> Self {
        let processing = pre_process(tree.clone(), meta_data.clone());
        let meta_data_clone = meta_data.clone();
        let future = Box::pin(async move {
            let mut scope: TokioScope<'a, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>> =
                unsafe { Scope::create() };
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
            // The rust compiler struggles to prove that the tokio-scope lifetime is the same as
            // the futures lifetime and errors. the transmute is simply casting the
            // lifetime to what it truly is. This is totally safe and will never cause
            // an error
            inspectors_execution: unsafe { std::mem::transmute(future) },
            pre_processing:       processing,
            metadata:             meta_data_clone,
        }
    }

    fn on_orchestra_resolution(
        &mut self,
        orchestra_data: Vec<(ClassifiedMev, Box<dyn SpecificMev>)>,
    ) -> Poll<ComposerResults> {
        info!("starting to compose classified mev");
        let mut header =
            build_mev_header(self.metadata.clone(), &self.pre_processing, &orchestra_data);

        let mut sorted_mev = sort_mev_by_type(orchestra_data);

        MEV_COMPOSABILITY_FILTER
            .iter()
            .for_each(|(head_mev_type, compose_fn, dependencies)| {
                self.try_compose_mev(head_mev_type, dependencies, compose_fn, &mut sorted_mev);
            });

        MEV_DEDUPLICATION_FILTER
            .iter()
            .for_each(|(head_mev_type, dependencies)| {
                self.deduplicate_mev(head_mev_type, dependencies, &mut sorted_mev);
            });

        let flattened_mev = sorted_mev
            .into_values()
            .flatten()
            .filter(|(classified, _)| classified.finalized_profit_usd > 0.0)
            .collect::<Vec<_>>();

        // set the mev count now that all reductions and merges have been made
        let mev_count = flattened_mev.len();
        header.mev_count = mev_count as u64;

        // downcast all of the sorted mev results. should cleanup
        Poll::Ready((header, flattened_mev))
    }

    //TODO: Clean up this function
    fn deduplicate_mev(
        &mut self,
        head_mev_type: &MevType,
        deps: &[MevType],
        sorted_mev: &mut HashMap<MevType, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>>,
    ) {
        let Some(head_mev) = sorted_mev.get(head_mev_type) else { return };
        let flattened_indexes = head_mev
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

        for (mev_type, index) in flattened_indexes {
            let entry = sorted_mev.get_mut(&mev_type).unwrap();
            if entry.len() > index {
                entry.remove(index);
            }
        }
    }

    /// Attempts to compose a new complex MEV occurrence from a list of
    /// MEV types that can be composed together.
    ///
    /// # Functionality:
    ///
    /// The function first checks if there are any MEV of the first type in
    /// `composable_types` in `sorted_mev`. If there are, it iterates over them.
    /// For each MEV, it gets the transaction hashes associated with that MEV
    /// and attempts to find other MEV in `sorted_mev` that have matching
    /// transaction hashes. If it finds matching MEV for all types in
    /// `composable_types`, it uses the `compose` function to create a new MEV
    /// and adds it to `sorted_mev` under `parent_mev_type`. It also records the
    /// indices of the composed MEV in `removal_indices`.
    ///
    /// After attempting to compose MEV for all MEV of the first type in
    /// `composable_types`, it removes all the composed MEV from `sorted_mev`
    /// using the indices stored in `removal_indices`.
    ///
    /// This function does not return any value. Its purpose is to modify
    /// `sorted_mev` by composing new MEV and removing the composed MEV.
    fn try_compose_mev(
        &mut self,
        parent_mev_type: &MevType,
        composable_types: &[MevType],
        compose: &ComposeFunction,
        sorted_mev: &mut HashMap<MevType, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>>,
    ) {
        let first_mev_type = composable_types[0];
        let mut removal_indices: HashMap<MevType, Vec<usize>> = HashMap::new();

        if let Some(first_mev_list) = sorted_mev.remove(&first_mev_type) {
            for (classified, mev_data) in first_mev_list {
                let tx_hashes = mev_data.mev_transaction_hashes();
                let mut to_compose = vec![(classified, mev_data.into_any())];
                let mut temp_removal_indices = Vec::new();

                for &other_mev_type in composable_types.iter().skip(1) {
                    if let Some(other_mev_data_list) = sorted_mev.get(&other_mev_type) {
                        match find_mev_with_matching_tx_hashes(other_mev_data_list, &tx_hashes) {
                            Some(index) => {
                                let (other_classified, other_mev_data) =
                                    &other_mev_data_list[index];
                                to_compose.push((
                                    other_classified.clone(),
                                    other_mev_data.clone().into_any(),
                                ));
                                temp_removal_indices.push((other_mev_type, index));
                            }
                            None => break,
                        }
                    } else {
                        break;
                    }
                }

                if to_compose.len() == composable_types.len() {
                    sorted_mev
                        .entry(*parent_mev_type)
                        .or_default()
                        .push(compose(to_compose));
                    for (mev_type, index) in temp_removal_indices {
                        removal_indices.entry(mev_type).or_default().push(index);
                    }
                }
            }
        }

        // Remove the mev data that was composed from the sorted mev list
        for (mev_type, indices) in removal_indices {
            if let Some(mev_list) = sorted_mev.get_mut(&mev_type) {
                for &index in indices.iter().rev() {
                    mev_list.remove(index);
                }
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

//TODO: Move to the database crate & track each block
// So for the master inspector we should get the address of the vertically
// integrated builders and know searcher addresses so we can also see when they
// are unprofitable and also better account for the profit given that they could
// be camouflaging thier trade by overbribing the builder given that
// they are one and the same
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
        sandwich::LongTailInspector,
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

        let (tokens_missing_decimals, tree) = classifier.build_block_tree(block.0, block.1);

        let USDC = Address::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap();

        // Here the quote address is USDC
        let cex_dex = Box::new(CexDexInspector::new(USDC)) as Box<dyn Inspector>;
        let backrun = Box::new(AtomicBackrunInspector::new(USDC)) as Box<dyn Inspector>;
        let jit = Box::new(JitInspector::new(USDC)) as Box<dyn Inspector>;
        let sandwich = Box::new(LongTailInspector::new(USDC)) as Box<dyn Inspector>;

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
