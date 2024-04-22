use std::collections::HashSet;

use brontes_types::{
    db::token_info::TokenInfoWithAddress,
    normalized_actions::{
        Actions, MultiCallFrameClassification, MultiFrameAction, MultiFrameRequest, Repayment,
    },
    Protocol, ToScaledRational, TreeSearchBuilder,
};
use tracing::warn;

use crate::multi_frame_classification::MultiCallFrameClassifier;

pub struct BancorV3;

impl MultiCallFrameClassifier for BancorV3 {
    const KEY: [u8; 2] = [Protocol::BancorV3 as u8, MultiFrameAction::FlashLoan as u8];

    fn create_classifier(
        request: MultiFrameRequest,
    ) -> Option<MultiCallFrameClassification<Actions>> {
        Some(MultiCallFrameClassification {
            trace_index:         request.trace_idx,
            tree_search_builder: TreeSearchBuilder::new().with_actions([
                Actions::is_swap,
                Actions::is_transfer,
                Actions::is_eth_transfer,
            ]),
            parse_fn:            Box::new(|this_action, child_nodes| {
                let this = this_action.try_flash_loan_mut().unwrap();
                let mut nodes_to_prune = Vec::new();
                let mut repay_transfers = Vec::new();
                let mut unique_transfers = HashSet::new();

                for (index, action) in child_nodes.into_iter() {
                    match &action {
                        Actions::Swap(_) | Actions::SwapWithFee(_) => {
                            this.child_actions.push(action);
                            nodes_to_prune.push(index);
                        }
                        Actions::Transfer(t) => {
                            let transfer_key = (t.from, t.to, t.token.address, t.amount.clone());
                            if this.pool == t.to
                                && this.assets.iter().any(|x| *x == t.token)
                                && this.amounts.iter().any(|amount| t.amount >= *amount)
                                && unique_transfers.insert(transfer_key)
                            {
                                repay_transfers.push(Repayment::Token(t.clone()));
                                nodes_to_prune.push(index);
                                continue;
                            }
                            this.child_actions.push(action);
                        }
                        Actions::EthTransfer(t) => {
                            let transfer_key = (
                                t.from,
                                t.to,
                                TokenInfoWithAddress::weth().address,
                                t.value.to_scaled_rational(18),
                            );
                            if this.pool == t.to
                                && this
                                    .assets
                                    .iter()
                                    .any(|x| *x == TokenInfoWithAddress::weth())
                                && this
                                    .amounts
                                    .iter()
                                    .any(|amount| t.value.to_scaled_rational(18) >= *amount)
                                && unique_transfers.insert(transfer_key)
                            {
                                repay_transfers.push(Repayment::Eth(t.clone()));
                                nodes_to_prune.push(index);
                                continue;
                            }
                            this.child_actions.push(action);
                        }
                        _ => {
                            warn!("Bancor V3 flashloan, unknown call: {:?}", action);
                        }
                    }
                }

                this.fees_paid = vec![];
                this.repayments = repay_transfers;

                nodes_to_prune
            }),
        })
    }
}
