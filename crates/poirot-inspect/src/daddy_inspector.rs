use std::{
    any::Any,
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll}
};

use futures::{
    future::{join_all, JoinAll},
    FutureExt, Stream
};
use malachite::{num::conversion::traits::RoundingFrom, rounding_modes::RoundingMode, Rational};
use poirot_labeller::Metadata;
use poirot_types::{
    classified_mev::{
        compose_sandwich_jit, ClassifiedMev, MevBlock, MevResult, MevType, SpecificMev
    },
    normalized_actions::Actions,
    tree::TimeTree
};
use reth_primitives::Address;

use crate::Inspector;

pub struct BlockPreprocessing {
    meta_data:           Arc<Metadata>,
    cumulative_gas_used: u64,
    cumulative_gas_paid: u64,
    builder_address:     Address
}

/// we use this to define a filter that we can iterate over such that
/// everything is ordered properly and we have already composed lower level
/// actions that could effect the higher level composing.
macro_rules! mev_composability {

    ($($mev_type:ident => $($deps:ident),+ => $replace:expr;)+) => {
        const MEV_FILTER: &'static [(
                MevType,
                Option<Box<dyn Fn(Box<dyn Any>, Box<dyn Any>, ClassifiedMev, ClassifiedMev) ->
                (ClassifiedMev, Box<dyn SpecificMev>)>>,
                &'static[MevType])] = &[
            $((MevType::$mev_type, $replace, &[$(MevType::$deps,)+]),)+
        ];
    };
}

mev_composability!(
    // reduce first
    Sandwich => Backrun => None;
    // try compose
    JitSandwich => Sandwich, Jit => Some(Box::new(compose_sandwich_jit));
);

type InspectorFut<'a> =
    JoinAll<Pin<Box<dyn Future<Output = Vec<(ClassifiedMev, Box<dyn SpecificMev>)>> + Send + 'a>>>;

/// the results downcast using any in order to be able to serialize and
/// impliment row trait due to the abosulte autism that the db library
/// requirements
pub type DaddyInspectorResults = (MevBlock, HashMap<MevType, Vec<(ClassifiedMev, MevResult)>>);

pub struct DaddyInspector<'a, const N: usize> {
    baby_inspectors:      &'a [&'a Box<dyn Inspector>; N],
    inspectors_execution: Option<InspectorFut<'a>>,
    pre_processing:       Option<BlockPreprocessing>
}

impl<'a, const N: usize> DaddyInspector<'a, N> {
    pub fn new(baby_inspectors: &'a [&'a Box<dyn Inspector>; N]) -> Self {
        Self { baby_inspectors, inspectors_execution: None, pre_processing: None }
    }

    pub fn is_processing(&self) -> bool {
        self.inspectors_execution.is_some()
    }

    pub fn on_new_tree(&mut self, tree: Arc<TimeTree<Actions>>, meta_data: Arc<Metadata>) {
        self.inspectors_execution = Some(join_all(
            self.baby_inspectors
                .iter()
                .map(|inspector| inspector.process_tree(tree.clone(), meta_data.clone()))
        ) as InspectorFut<'a>);

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
            builder_address
        });
    }

    fn build_mev_header(
        &mut self,
        baby_data: Arc<impl Iterator<Item = (ClassifiedMev, Box<dyn SpecificMev>)>>
    ) -> MevBlock {
        let pre_processing = self.pre_processing.take().unwrap();
        let cum_mev_priority_fee_paid = baby_data
            .clone()
            .map(|(_, mev)| mev.priority_fee_paid())
            .sum::<u64>();

        let total_bribe = 0;

        let builder_eth_profit = total_bribe + pre_processing.cumulative_gas_paid;

        MevBlock {
            block_hash: pre_processing.meta_data.block_hash.into(),
            block_number: pre_processing.meta_data.block_num,
            mev_count: baby_data.count() as u64,
            submission_eth_price: f64::rounding_from(
                pre_processing.meta_data.eth_prices.0,
                RoundingMode::Nearest
            )
            .0,
            finalized_eth_price: f64::rounding_from(
                pre_processing.meta_data.eth_prices.1,
                RoundingMode::Nearest
            )
            .0,
            cumulative_gas_used: pre_processing.cumulative_gas_used,
            cumulative_gas_paid: pre_processing.cumulative_gas_paid,
            total_bribe: baby_data.map(|(_, mev)| mev.bribe()).sum::<u64>(),
            cumulative_mev_priority_fee_paid: cum_mev_priority_fee_paid,
            builder_address: pre_processing.builder_address,
            builder_eth_profit,
            builder_submission_profit_usd: f64::rounding_from(
                Rational::from(builder_eth_profit) * pre_processing.meta_data.eth_prices.0,
                RoundingMode::Nearest
            )
            .0,
            builder_finalized_profit_usd: f64::rounding_from(
                Rational::from(builder_eth_profit) * pre_processing.meta_data.eth_prices.1,
                RoundingMode::Nearest
            )
            .0,
            proposer_fee_recipient: pre_processing.meta_data.proposer_fee_recipient,
            proposer_mev_reward: pre_processing.meta_data.proposer_mev_reward,
            proposer_submission_mev_reward_usd: f64::rounding_from(
                Rational::from(pre_processing.meta_data.proposer_mev_reward)
                    * pre_processing.meta_data.eth_prices.0,
                RoundingMode::Nearest
            )
            .0,
            proposer_finalized_mev_reward_usd: f64::rounding_from(
                Rational::from(pre_processing.meta_data.proposer_mev_reward)
                    * pre_processing.meta_data.eth_prices.1,
                RoundingMode::Nearest
            )
            .0,
            cumulative_mev_submission_profit_usd: f64::rounding_from(
                Rational::from(cum_mev_priority_fee_paid) * pre_processing.meta_data.eth_prices.0,
                RoundingMode::Nearest
            )
            .0,
            cumulative_mev_finalized_profit_usd: f64::rounding_from(
                Rational::from(cum_mev_priority_fee_paid) * pre_processing.meta_data.eth_prices.1,
                RoundingMode::Nearest
            )
            .0
        }
    }

    fn on_baby_resolution(
        &mut self,
        baby_data: impl Iterator<Item = (ClassifiedMev, Box<dyn SpecificMev>)>
    ) -> Poll<Option<DaddyInspectorResults>> {
        let header = self.build_mev_header(Arc::new(baby_data));

        let mut sorted_mev = baby_data
            .map(|(classified_mev, specific)| (classified_mev.mev_type, (classified_mev, specific)))
            .collect::<HashMap<MevType, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>>>();

        MEV_FILTER
            .iter()
            .for_each(|(head_mev_type, compose_fn, dependencies)| {
                if let Some(compose_fn) = compose_fn {
                    self.compose_dep_filter(
                        head_mev_type,
                        dependencies,
                        compose_fn,
                        &mut sorted_mev
                    );
                } else {
                    self.replace_dep_filter(head_mev_type, dependencies, &mut sorted_mev);
                }
            });

        // downcast all of the sorted mev results
        Poll::Ready(Some(
            sorted_mev
                .into_iter()
                .map(|(k, v)| {
                    let new_v = v
                        .into_iter()
                        .map(|(class, mut other)| {
                            let mut a_ref = other.as_mut();
                            let any_cast = &mut a_ref as &mut dyn Any;

                            match k {
                                MevType::Jit => {
                                    MevResult::Jit(any_cast.downcast_mut().cloned().unwrap())
                                }
                                MevType::CexDex => {
                                    MevResult::CexDex(any_cast.downcast_mut().cloned().unwrap())
                                }
                                MevType::Sandwich => {
                                    MevResult::Sandwich(any_cast.downcast_mut().cloned().unwrap())
                                }
                                MevType::JitSandwich => MevResult::JitSandwich(
                                    any_cast.downcast_mut().cloned().unwrap()
                                ),
                                MevType::Liquidation => MevResult::Liquidation(
                                    any_cast.downcast_mut().cloned().unwrap()
                                ),
                                _ => todo!("add other downcasts for different types")
                            }
                        })
                        .collect::<Vec<_>>();
                    (k, new_v)
                })
                .collect()
        ))
    }

    fn replace_dep_filter(
        &mut self,
        head_mev_type: &MevType,
        deps: &[MevType],
        sorted_mev: &mut HashMap<MevType, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>>
    ) {
        let Some(head_mev) = sorted_mev.get(head_mev_type) else { return };

        let mut remove_count: HashMap<MevType, usize> = HashMap::new();

        let mut flattend_indexes = head_mev
            .iter()
            .flat_map(|(classified, specific)| {
                let hashes = specific.mev_transaction_hashes();
                let mut remove_data: Vec<(MevType, usize)> = Vec::new();
                for dep in deps {
                    let Some(dep_mev) = sorted_mev.get(dep) else { continue };
                    for (i, (class, specific)) in dep_mev.iter().enumerate() {
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
        // we know this has len 2
        composable_types: &[MevType],
        compose: &Box<
            dyn Fn(
                Box<dyn Any>,
                Box<dyn Any>,
                ClassifiedMev,
                ClassifiedMev
            ) -> (ClassifiedMev, Box<dyn SpecificMev>)
        >,
        sorted_mev: &mut HashMap<MevType, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>>
    ) {
        if composable_types.len() != 2 {
            panic!("we only support sequential compatibility for our specific mev");
        }

        let mut zero_txes = sorted_mev.remove(&composable_types[0]).unwrap();
        let mut one_txes = sorted_mev.get(&composable_types[1]).unwrap();
        for (classified, mev_data) in zero_txes {
            let addresses = mev_data.mev_transaction_hashes();

            if let Some((index, _)) =
                one_txes
                    .iter()
                    .enumerate()
                    .map(|(i, d)| (i, d))
                    .find(|(_, (k, v))| {
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
                        classifed_1
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

impl<const N: usize> Stream for DaddyInspector<'_, N> {
    type Item = DaddyInspectorResults;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(mut calculations) = self.inspectors_execution.take() {
            return match calculations.poll_unpin(cx) {
                Poll::Ready(data) => self.on_baby_resolution(data.into_iter().flatten()),
                Poll::Pending => {
                    self.inspectors_execution = Some(calculations);
                    Poll::Pending
                }
            }
        }
        Poll::Pending
    }
}
