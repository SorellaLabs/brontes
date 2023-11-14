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
};
use futures::FutureExt;
use lazy_static::lazy_static;
use malachite::{num::conversion::traits::RoundingFrom, rounding_modes::RoundingMode, Rational};
use reth_primitives::Address;
use tracing::info;

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
    Sandwich => Backrun;
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
    cumulative_gas_used: u64,
    cumulative_gas_paid: u64,
    builder_address:     Address,
}

type InspectorFut<'a> =
    Pin<Box<dyn Future<Output = Vec<(ClassifiedMev, Box<dyn SpecificMev>)>> + 'a>>;

/// the results downcast using any in order to be able to serialize and
/// impliment row trait due to the abosulte autism that the db library   
/// requirements
pub type ComposerResults = (MevBlock, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>);

pub struct Composer<'a, const N: usize> {
    orchestra:            &'a [&'a Box<dyn Inspector>; N],
    inspectors_execution: Option<InspectorFut<'a>>,
    pre_processing:       Option<BlockPreprocessing>,
    // this is terroristic and need to prob rewrite most of this. however
    // we will leave it for now so we can get to testing
    is_finished:          bool,
}

impl<'a, const N: usize> Composer<'a, N> {
    pub fn new(orchestra: &'a [&'a Box<dyn Inspector>; N]) -> Self {
        Self { orchestra, inspectors_execution: None, pre_processing: None, is_finished: false }
    }

    pub fn is_processing(&self) -> bool {
        self.inspectors_execution.is_some()
    }

    pub fn is_finished(&self) -> bool {
        return self.is_finished
    }

    pub fn on_new_tree(&mut self, tree: Arc<TimeTree<Actions>>, meta_data: Arc<Metadata>) {
        // This is only unsafe due to the fact that you can have missbehaviour where you
        // drop this with incomplete futures
        let mut scope: TokioScope<'_, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>> =
            unsafe { Scope::create() };

        println!("inspectors to run: {}", self.orchestra.len());
        self.orchestra.iter().for_each(|inspector| {
            scope.spawn(inspector.process_tree(tree.clone(), meta_data.clone()))
        });

        let fut = Box::pin(async move {
            scope
                .collect()
                .map(|r| r.into_iter().flatten().flatten().collect::<Vec<_>>())
                .await
        });

        self.inspectors_execution = Some(fut);

        self.pre_process(tree, meta_data);
    }

    fn pre_process(&mut self, tree: Arc<TimeTree<Actions>>, meta_data: Arc<Metadata>) {
        let builder_address = tree.header.beneficiary;
        let cumulative_gas_used = tree
            .roots
            .iter()
            .map(|root| root.gas_details.gas_used)
            .sum::<u64>();

        let cumulative_gas_paid = tree
            .roots
            .iter()
            .map(|root| root.gas_details.effective_gas_price * root.gas_details.gas_used)
            .sum::<u64>();

        self.pre_processing = Some(BlockPreprocessing {
            meta_data,
            cumulative_gas_used,
            cumulative_gas_paid,
            builder_address,
        });
    }

    fn build_mev_header(
        &mut self,
        orchestra_data: &Vec<(ClassifiedMev, Box<dyn SpecificMev>)>,
    ) -> MevBlock {
        let pre_processing = self.pre_processing.take().unwrap();
        let cum_mev_priority_fee_paid = orchestra_data
            .iter()
            .map(|(_, mev)| mev.priority_fee_paid())
            .sum::<u64>();

        let total_bribe = 0;

        let builder_eth_profit = total_bribe + pre_processing.cumulative_gas_paid;

        MevBlock {
            block_hash: pre_processing.meta_data.block_hash.into(),
            block_number: pre_processing.meta_data.block_num,
            mev_count: orchestra_data.len() as u64,
            submission_eth_price: f64::rounding_from(
                &pre_processing.meta_data.eth_prices.0,
                RoundingMode::Nearest,
            )
            .0,
            finalized_eth_price: f64::rounding_from(
                &pre_processing.meta_data.eth_prices.1,
                RoundingMode::Nearest,
            )
            .0,
            cumulative_gas_used: pre_processing.cumulative_gas_used,
            cumulative_gas_paid: pre_processing.cumulative_gas_paid,
            total_bribe: orchestra_data
                .iter()
                .map(|(_, mev)| mev.bribe())
                .sum::<u64>(),
            cumulative_mev_priority_fee_paid: cum_mev_priority_fee_paid,
            builder_address: pre_processing.builder_address,
            builder_eth_profit,
            builder_submission_profit_usd: f64::rounding_from(
                Rational::from(builder_eth_profit) * &pre_processing.meta_data.eth_prices.0,
                RoundingMode::Nearest,
            )
            .0,
            builder_finalized_profit_usd: f64::rounding_from(
                Rational::from(builder_eth_profit) * &pre_processing.meta_data.eth_prices.1,
                RoundingMode::Nearest,
            )
            .0,
            proposer_fee_recipient: pre_processing.meta_data.proposer_fee_recipient,
            proposer_mev_reward: pre_processing.meta_data.proposer_mev_reward,
            proposer_submission_profit_usd: f64::rounding_from(
                Rational::from(pre_processing.meta_data.proposer_mev_reward)
                    * &pre_processing.meta_data.eth_prices.0,
                RoundingMode::Nearest,
            )
            .0,
            proposer_finalized_profit_usd: f64::rounding_from(
                Rational::from(pre_processing.meta_data.proposer_mev_reward)
                    * &pre_processing.meta_data.eth_prices.1,
                RoundingMode::Nearest,
            )
            .0,
            cumulative_mev_submission_profit_usd: f64::rounding_from(
                Rational::from(cum_mev_priority_fee_paid) * &pre_processing.meta_data.eth_prices.0,
                RoundingMode::Nearest,
            )
            .0,
            cumulative_mev_finalized_profit_usd: f64::rounding_from(
                Rational::from(cum_mev_priority_fee_paid) * &pre_processing.meta_data.eth_prices.1,
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

        self.is_finished = true;

        // downcast all of the sorted mev results. should cleanup
        Poll::Ready((header, sorted_mev.into_values().flatten().collect::<Vec<_>>()))
    }

    fn replace_dep_filter(
        &mut self,
        head_mev_type: &MevType,
        deps: &[MevType],
        sorted_mev: &mut HashMap<MevType, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>>,
    ) {
        let Some(head_mev) = sorted_mev.get(head_mev_type) else { return };

        let mut remove_count: HashMap<MevType, usize> = HashMap::new();

        let flattend_indexes = head_mev
            .iter()
            .flat_map(|(_, specific)| {
                let hashes = specific.mev_transaction_hashes();
                let mut remove_data: Vec<(MevType, usize)> = Vec::new();
                for dep in deps {
                    let Some(dep_mev) = sorted_mev.get(dep) else { continue };
                    for (i, (_, specific)) in dep_mev.iter().enumerate() {
                        let dep_hashes = specific.mev_transaction_hashes();
                        // verify both match
                        if dep_hashes == hashes {
                            let adjustment = remove_count.entry(*dep).or_default();
                            remove_data.push((*dep, i - *adjustment));
                            *adjustment += 1;
                        }
                        // we only want one match
                        else if dep_hashes
                            .iter()
                            .map(|hash| hashes.contains(hash))
                            .any(|f| f)
                        {
                            let adjustment = remove_count.entry(*dep).or_default();
                            remove_data.push((*dep, i + *adjustment));
                            *adjustment += 1;
                        }
                    }
                }

                remove_data
            })
            .collect::<Vec<(MevType, usize)>>();

        for (mev_type, index) in flattend_indexes {
            sorted_mev.get_mut(&mev_type).unwrap().remove(index);
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

            if let Some((index, _)) = sorted_mev
                .get(&composable_types[1])
                .unwrap()
                .iter()
                .enumerate()
                .find(|(_, (_, v))| {
                    let o_addrs = v.mev_transaction_hashes();
                    o_addrs == addresses || addresses.iter().any(|a| o_addrs.contains(a))
                })
            {
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
        if let Some(mut calculations) = self.inspectors_execution.take() {
            return match calculations.poll_unpin(cx) {
                Poll::Ready(data) => self.on_orchestra_resolution(data),
                Poll::Pending => {
                    self.inspectors_execution = Some(calculations);
                    Poll::Pending
                }
            }
        }
        Poll::Pending
    }
}
