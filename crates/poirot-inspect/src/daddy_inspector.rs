use std::{sync::Arc, task::Poll};

use reth_rpc_types::Action;
use poirot_labeller::Metadata;
use poirot_types::{
    classified_mev::{ClassifiedMev, MevBlock, SpecificMev},
    normalized_actions::Actions,
    tree::TimeTree
};
use reth_primitives::Address;

type InspectorFut<'a> =
    JoinAll<Pin<Box<dyn Future<Output = Vec<(ClassifiedMev, Box<dyn SpecificMev>)>> + Send + 'a>>>;

pub type DaddyInspectorResults = (MevBlock, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>);

pub struct BlockPreprocessing {
    meta_data:           Arc<Metadata>,
    cumulative_gas_used: u64,
    cumulative_gas_paid: u64,
    builder_address:     Address,
    proposer_address:    Address,

    proposer_mev_reward:                u64,
    proposer_submission_mev_reward_usd: u64,
    proposer_finalized_mev_reward_usd:  u64
}

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

        let proposer_address = tree.roots.last().unwrap().inspect(&|node|{
            node.subactions.iter().any(|action| {
                if let Actions::Unclassified(unclassified, _) = action {
                    if let Action::Call(call) = unclassified.action {
                    }

                }
            })
        });
    }

    fn on_baby_resolution(
        &mut self,
        baby_data: Vec<Vec<(ClassifiedMev, Box<dyn SpecificMev>)>>
    ) -> Poll<Option<DaddyInspectorResults>> {
        let mut mev_block = MevBlock {
            block_hash: (),
            block_number: (),
            mev_count: (),
            submission_eth_price: (),
            finalized_eth_price: (),
            /// Gas
            cumulative_gas_used: (),
            cumulative_gas_paid: (),
            total_bribe: (),
            cumulative_mev_priority_fee_paid: (),
            /// Builder address (recipient of coinbase.transfers)
            builder_address: (),
            builder_eth_profit: (),
            builder_submission_profit_usd: (),
            builder_finalized_profit_usd: (),
            /// Proposer address
            proposer_fee_recipient: (),
            proposer_mev_reward: (),
            proposer_submission_mev_reward_usd: (),
            proposer_finalized_mev_reward_usd: (),
            // gas used * (effective gas price - base fee) for all Classified MEV txs
            /// Mev profit
            cumulative_mev_submission_profit_usd: (),
            cumulative_mev_finalized_profit_usd: ()
        };
    }
}

impl<const N: usize> Stream for DaddyInspector<'_, N> {
    type Item = DaddyInspectorResults;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(mut calculations) = self.inspectors_execution.take() {
            match calculations.poll_next_unpin(cx) {
                Poll::Ready(data) => self.on_baby_resolution(data),
                Poll::Pending => {
                    self.inspectors_execution = Some(calculations);
                    Poll::Pending
                }
            }
        }
    }
}
