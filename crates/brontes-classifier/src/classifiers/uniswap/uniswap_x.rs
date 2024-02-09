use alloy_primitives::U256;
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    db::token_info::TokenInfoWithAddress,
    normalized_actions::{NormalizedBatch, NormalizedSwap},
    structured_trace::CallInfo,
};
use malachite::Rational;

use crate::UniswapX::Fill;

action_impl!(
    Protocol::UniswapX,
    crate::UniswapX::executeCall,
    Batch,
    [..Fill*],
    call_data: true,
    logs: true,
    |
    info: CallInfo,
    _call_data: executeCall,
    logs_data: UniswapXexecuteCallLogs,
    _db_tx: &DB| {

        let fill_logs = logs_data.Fill_field;

        let solver = fill_logs[0].filler;

        Ok(NormalizedBatch {
            protocol: Protocol::UniswapX,
            trace_index: info.trace_idx,
            solver,
            settlement_contract: info.target_address,
            user_swaps: fill_logs.iter().map(Fill::into_swap).collect(),
            solver_swaps: None,
            msg_value: info.msg_value

        })
    }
);

action_impl!(
    Protocol::UniswapX,
    crate::UniswapX::executeBatchCall,
    Batch,
    [..Fill*],
    logs: true,
    call_data: true,
    |
    info: CallInfo,
    _call_data: executeBatchCall,
    logs_data: UniswapXexecuteBatchCallLogs,
    _db_tx: &DB| {

        let fill_logs = logs_data.Fill_field;

        let solver = fill_logs[0].filler;

        Ok(NormalizedBatch {
            protocol: Protocol::UniswapX,
            trace_index: info.trace_idx,
            solver,
            settlement_contract: info.target_address,
            user_swaps: fill_logs.iter().map(Fill::into_swap).collect(),
            solver_swaps: None,
            msg_value: info.msg_value,
        })
    }
);

action_impl!(
    Protocol::UniswapX,
    crate::UniswapX::executeBatchWithCallbackCall,
    Batch,
    [Fill*],
    call_data: true,
    logs: true,
    |
    info: CallInfo,
    _call_data: executeBatchWithCallbackCall,
    logs_data: UniswapXexecuteBatchWithCallbackCallLogs,
    _db_tx: &DB| {
        let fill_logs = logs_data.Fill_field;


        let solver = fill_logs[0].filler;

        Ok(NormalizedBatch {
            protocol: Protocol::UniswapX,
            trace_index: info.trace_idx,
            solver,
            settlement_contract: info.target_address,
            user_swaps: fill_logs.iter().map(Fill::into_swap).collect(),
            solver_swaps: Some(Vec::new()),
            msg_value: info.msg_value
        })
    }
);

action_impl!(
    Protocol::UniswapX,
    crate::UniswapX::executeWithCallbackCall,
    Batch,
    [Fill*],
    call_data: true,
    logs: true,
    |
    info: CallInfo,
    _call_data: executeWithCallbackCall,
    logs_data: UniswapXexecuteWithCallbackCallLogs,
    _db_tx: &DB| {
        let fill_logs = logs_data.Fill_field;

        let solver = fill_logs[0].filler;



        Ok(NormalizedBatch {
            protocol: Protocol::UniswapX,
            trace_index: info.trace_idx,
            solver,
            settlement_contract: info.target_address,
            user_swaps: fill_logs.iter().map(Fill::into_swap).collect(),
            solver_swaps: None,
            msg_value: info.msg_value
        })
    }
);

impl Fill {
    /// Here we're converting a Fill into a NormalizedSwap, however we don't yet
    /// have the full trade information. We'll fill this in at the final
    /// classification stage. See: [`Finish
    /// Classification`](brontes_types::NormalizedBatch::normalized_actions::finish_classification)
    pub fn into_swap(fill_log: &Fill) -> NormalizedSwap {
        let solver = fill_log.filler;
        let swapper = fill_log.swapper;

        NormalizedSwap {
            protocol:    Protocol::UniswapX,
            trace_index: 0,
            from:        swapper,
            recipient:   swapper,
            pool:        solver,
            token_in:    TokenInfoWithAddress::default(),
            token_out:   TokenInfoWithAddress::default(),
            amount_in:   Rational::default(),
            amount_out:  Rational::default(),
            msg_value:   U256::ZERO,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{hex, Address, B256, U256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_pricing::Protocol::UniswapX;
    use brontes_types::{normalized_actions::Actions, Node, ToScaledRational, TreeSearchArgs};
    use serial_test::serial;

    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_batch_classifier_with_call_back_eth() {
        let classifier_utils = ClassifierTestUtils::new();
        let execute_batch_with_callback =
            B256::from(hex!("3d8fbccb1b0b7f8140f255f0980d897d87394903ad7bf4d08534402d2bf35872"));

        let eq_action = Actions::Batch(NormalizedBatch {
            protocol:            Protocol::UniswapX,
            trace_index:         1,
            solver:              Address::new(hex!(
                "
            919f9173E2Dc833Ec708812B4f1CB11B1a17eFDe"
            )),
            settlement_contract: Address::new(hex!("6000da47483062A0D734Ba3dc7576Ce6A0B645C4")),
            user_swaps:          vec![
                NormalizedSwap {
                    protocol:    UniswapX,
                    trace_index: 2,
                    from:        Address::new(hex!(
                        "
            86C2c32cea0F9cb6ef9742a138D0D4843598d0d6"
                    )),
                    recipient:   Address::new(hex!(
                        "
                    86C2c32cea0F9cb6ef9742a138D0D4843598d0d6"
                    )),
                    pool:        Address::new(hex!(
                        "
                    919f9173e2dc833ec708812b4f1cb11b1a17efde"
                    )),
                    token_in:    TokenInfoWithAddress::usdt(),
                    amount_in:   U256::from_str("2400058669").unwrap().to_scaled_rational(6),
                    token_out:   TokenInfoWithAddress::native_eth(),
                    amount_out:  U256::from_str("1045065358997285550")
                        .unwrap()
                        .to_scaled_rational(18),

                    msg_value: U256::ZERO,
                },
                NormalizedSwap {
                    protocol:    UniswapX,
                    trace_index: 3,
                    from:        Address::new(hex!(
                        "
                    569d9f244e4ed4f0731f39675492740dcdab6b15"
                    )),
                    recipient:   Address::new(hex!(
                        "
                    569d9f244e4ed4f0731f39675492740dcdab6b15"
                    )),
                    pool:        Address::new(hex!(
                        "
                    919f9173e2dc833ec708812b4f1cb11b1a17efde"
                    )),
                    token_in:    TokenInfoWithAddress::usdt(),
                    amount_in:   U256::from_str("106496770").unwrap().to_scaled_rational(6),
                    token_out:   TokenInfoWithAddress::native_eth(),
                    amount_out:  U256::from_str("1569952967947850")
                        .unwrap()
                        .to_scaled_rational(18),
                    msg_value:   U256::ZERO,
                },
            ],
            solver_swaps:        None,
            msg_value:           U256::ZERO,
        });

        let search_fn = |node: &Node<Actions>| TreeSearchArgs {
            collect_current_node:  node.data.is_batch(),
            child_node_to_collect: node.subactions.iter().any(|action| action.is_batch()),
        };

        classifier_utils
            .contains_action(execute_batch_with_callback, 0, eq_action, search_fn)
            .await
            .unwrap();
    }
}
