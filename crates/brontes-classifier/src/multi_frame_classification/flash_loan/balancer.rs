use brontes_types::{
    normalized_actions::{
        Action, MultiCallFrameClassification, MultiFrameAction, MultiFrameRequest,
    },
    Protocol, TreeSearchBuilder,
};
use tracing::warn;

use crate::multi_frame_classification::MultiCallFrameClassifier;

pub struct BalancerV2;

impl MultiCallFrameClassifier for BalancerV2 {
    const KEY: [u8; 2] = [Protocol::BalancerV2 as u8, MultiFrameAction::FlashLoan as u8];

    fn create_classifier(
        request: MultiFrameRequest,
    ) -> Option<MultiCallFrameClassification<Action>> {
        Some(MultiCallFrameClassification {
            trace_index:         request.trace_idx,
            tree_search_builder: TreeSearchBuilder::new().with_actions([
                Action::is_swap,
                Action::is_transfer,
                Action::is_eth_transfer,
            ]),
            parse_fn:            Box::new(|this_action, child_nodes| {
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
                                        continue
                                    }
                                }
                            }
                            this.child_actions.push(action);
                            nodes_to_prune.push(index);
                        }
                        _ => {
                            warn!("Balancer V2 flashloan, unknown call");
                            continue
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

#[cfg(test)]
mod tests {
    use std::{iter::FromIterator, str::FromStr, sync::Arc};

    use alloy_primitives::{hex, Address, B256};
    use brontes_core::decoding::{Parser, TracingProvider};
    use brontes_types::{
        db::token_info::{TokenInfo, TokenInfoWithAddress},
        normalized_actions::Action,
        structured_trace::TxTrace,
        BlockTree, TreeSearchBuilder,
    };
    use eyre;
    use serde::{Deserialize, Serialize};
    use serde_json::{json, Value};

    use super::*;
    use crate::test_utils::ClassifierTestUtils;

    #[derive(Debug, Serialize, Deserialize)]
    struct JsonRpcRequest {
        jsonrpc: String,
        method:  String,
        params:  Value,
        id:      u64,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct JsonRpcResponse {
        jsonrpc: String,
        result:  Option<Value>,
        error:   Option<JsonRpcError>,
        id:      u64,
    }

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct TraceResult {
        tx_hash: B256,
        result:  TxTrace,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct JsonRpcError {
        code:    i64,
        message: String,
    }

    #[brontes_macros::test]
    async fn test_balncer_flashloan() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let json_response: Value = serde_json::from_str(include_str!("example.json")).unwrap();

        let json_string = json_response.to_string();
        let rpc_response: JsonRpcResponse = serde_json::from_str(&json_string).unwrap();

        let trace_result: Vec<TxTrace> =
            vec![serde_json::from_value(rpc_response.result.unwrap()).unwrap()];

        classifier_utils.ensure_token(TokenInfoWithAddress {
            address: Address::new(hex!("af88d065e77c8cc2239327c5edb3a432268e5831")),
            inner:   TokenInfo { decimals: 6, symbol: "USDC".to_string() },
        });

        let tree = Arc::new(
            classifier_utils
                .build_tree_tx(trace_result[0].tx_hash)
                .await
                .unwrap(),
        );

        let mut actions = tree
            .collect(
                &trace_result[0].tx_hash,
                TreeSearchBuilder::default().with_action(Action::is_flash_loan),
            )
            .collect::<Vec<_>>();

        println!("trace_result: {:?}", actions);
    }
}
