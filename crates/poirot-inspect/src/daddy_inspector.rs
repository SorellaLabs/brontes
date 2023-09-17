use std::{collections::HashMap, sync::Arc, task::Poll};

use poirot_labeller::Metadata;
use poirot_types::{
    classified_mev::{ClassifiedMev, MevBlock, MevType, SpecificMev},
    normalized_actions::Actions,
    tree::TimeTree
};
use reth_primitives::Address;
use reth_rpc_types::trace::parity::Action;
use strum::IntoEnumIterator;

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
        const MEV_FILTER: &'static [(MevType, bool, &'static[MevType])] = &[
            $((MevType::$mev_type, $replace, &[$(MevType::$deps,)+]),)+
        ];
    };
}

mev_composability!(
    Sandwich => Backrun => true;
    JitSandwich => Sandwich, Jit => false;
);

type InspectorFut<'a> =
    JoinAll<Pin<Box<dyn Future<Output = Vec<(ClassifiedMev, Box<dyn SpecificMev>)>> + Send + 'a>>>;

pub type DaddyInspectorResults = (MevBlock, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>);

pub struct DaddyInspector<'a, const N: usize> {
    baby_inspectors:      &'a [&'a Box<dyn Inspector<Mev = Box<dyn SpecificMev>>>; N],
    inspectors_execution: Option<InspectorFut<'a>>,
    pre_processing:       Option<BlockPreprocessing>
}

impl<'a, const N: usize> DaddyInspector<'a, N> {
    pub fn new(baby_inspectors: &'a [&'a Box<dyn Inspector<Mev = dyn SpecificMev>>; N]) -> Self {
        Self { baby_inspectors, inspectors_execution: None, pre_processing: None }
    }

    pub fn is_processing(&self) -> bool {
        self.inspectors_execution.is_some()
    }

    pub fn on_new_tree(&mut self, tree: Arc<TimeTree<Actions>>, meta_data: Arc<Metadata>) {
        self.inspectors_execution = Some(join_all(
            self.baby_inspectors
                .iter()
                .map(|inspector| inspector.process_tree(tree.clone(), metadata.clone()))
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
        baby_data: impl AsRef<Iterator<Item = (ClassifiedMev, Box<dyn SpecificMev>)>>
    ) -> MevBlock {
        let pre_processing = self.pre_processing.take().unwrap();
        let cum_mev_priority_fee_paid = baby_data
            .map(|(_, mev)| mev.priority_fee_paid())
            .sum::<u64>();

        let builder_eth_profit = (total_bribe + pre_processing.cumulative_gas_paid);

        let mut mev_block = MevBlock {
            block_hash: pre_processing.meta_data.block_hash,
            block_number: pre_processing.meta_data.block_num,
            mev_count: baby_data.count(),
            submission_eth_price: pre_processing.meta_data.eth_prices.0,
            finalized_eth_price: pre_processing.meta_data.eth_prices.1,
            cumulative_gas_used: pre_processing.cumulative_gas_used,
            cumulative_gas_paid: pre_processing.cumulative_gas_paid,
            total_bribe: baby_data.map(|(_, mev)| mev.bribe()).sum::<u64>(),
            cumulative_mev_priority_fee_paid: cum_mev_priority_fee_paid,
            builder_address: pre_processing.builder_address,
            builder_eth_profit,
            builder_submission_profit_usd: builder_eth_profit
                * pre_processing.meta_data.eth_prices.0,
            builder_finalized_profit_usd: builder_eth_profit
                * pre_processing.meta_data.eth_prices.1,
            proposer_fee_recipient: pre_processing.meta_data.proposer_fee_recipient,
            proposer_mev_reward: pre_processing.meta_data.proposer_mev_reward,
            proposer_submission_mev_reward_usd: pre_processing.meta_data.proposer_mev_reward
                * pre_processing.meta_data.eth_prices.0,
            proposer_finalized_mev_reward_usd: pre_processing.meta_data.proposer_mev_reward
                * pre_processing.meta_data.eth_prices.1,
            cumulative_mev_submission_profit_usd: cum_mev_priority_fee_paid
                * pre_processing.meta_data.eth_prices.0,
            cumulative_mev_finalized_profit_usd: cum_mev_priority_fee_paid
                * pre_processing.meta_data.eth_prices.1
        };
    }

    fn on_baby_resolution(
        &mut self,
        baby_data: impl Iterator<Item = (ClassifiedMev, Box<dyn SpecificMev>)>
    ) -> Poll<Option<DaddyInspectorResults>> {
        let header = self.build_mev_header(&baby_data);

        let mut sorted_mev = baby_data
            .map(|(classified_mev, specific)| (classified_mev.mev_type, (classified_mev, specific)))
            .collect::<HashMap<MevType, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>>>();

        MEV_FILTER
            .iter()
            .for_each(|(head_mev_type, replace, dependencies)| {
                if replace {
                    self.replace_dep_filter(head_mev_type, dependencies, &mut sorted_mev);
                } else {
                    self.compose_dep_filter(head_mev_type, dependencies, &mut sorted_mev);
                }
            });
    }

    fn replace_dep_filter(
        &mut self,
        head_mev_type: &MevType,
        deps: &&[MevType],
        sorted_mev: &mut HashMap<MevType, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>>
    ) {
        let Some(head_mev) = sorted_mev.get_mut(head_mev_type) else { return; };

        head_mev.iter_mut().for_each(|(classified, specific)| {
            let addresses = specific.mev_transaction_hashes();

            // lets check all deps for head to see if we can compose
            // shit
        });
    }

    fn compose_dep_filter(
        &mut self,
        head_mev_type: &MevType,
        deps: &&[MevType],
        sorted_mev: &mut HashMap<MevType, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>>
    ) {
    }
}

impl<const N: usize> Stream for DaddyInspector<'_, N> {
    type Item = DaddyInspectorResults;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(mut calculations) = self.inspectors_execution.take() {
            match calculations.poll_next_unpin(cx) {
                Poll::Ready(data) => self.on_baby_resolution(data.into_iter().flatten()),
                Poll::Pending => {
                    self.inspectors_execution = Some(calculations);
                    Poll::Pending
                }
            }
        }
    }
}
