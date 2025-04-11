use crate::multi_frame_classification::MultiCallFrameClassifier;
use brontes_types::TreeSearchFn;
use brontes_types::{
    normalized_actions::{
        Action, MultiCallFrameClassification, MultiFrameAction, MultiFrameRequest,
    },
    Protocol, TreeSearchBuilder,
};
use tracing::warn;

pub struct Dodo;

impl MultiCallFrameClassifier for Dodo {
    const KEY: [u8; 2] = [Protocol::Dodo as u8, MultiFrameAction::FlashLoan as u8];

    fn create_classifier(
        request: MultiFrameRequest,
    ) -> Option<MultiCallFrameClassification<Action>> {
        Some(MultiCallFrameClassification {
            trace_index: request.trace_idx,
            tree_search_builder: TreeSearchBuilder::new().with_actions([
                Action::is_swap.boxed(),
                Action::is_transfer.boxed(),
                Action::is_eth_transfer.boxed(),
            ]),
            parse_fn: Box::new(|this_action, child_nodes| {
                let this = this_action.try_flash_loan_mut().unwrap();
                let mut nodes_to_prune = Vec::new();
                let mut repay_transfers = Vec::new();

                for (index, action) in child_nodes.into_iter() {
                    match &action {
                        Action::Swap(_) | Action::SwapWithFee(_) | Action::EthTransfer(_) => {
                            this.child_actions.push(action);
                            nodes_to_prune.push(index);
                        }
                        Action::Transfer(t) => {
                            if t.from == this.receiver_contract && this.pool == t.to {
                                if let Some(i) = this.assets.iter().position(|x| *x == t.token) {
                                    if t.amount >= this.amounts[i] {
                                        repay_transfers.push(t.clone());
                                        nodes_to_prune.push(index);
                                        continue;
                                    }
                                }
                            }
                            this.child_actions.push(action);
                            nodes_to_prune.push(index);
                        }
                        _ => {
                            warn!("Dodo flashloan, unknown call");
                            continue;
                        }
                    }
                }

                // no fee
                this.fees_paid = vec![];
                this.repayments = repay_transfers;

                nodes_to_prune
            }),
        })
    }
}
