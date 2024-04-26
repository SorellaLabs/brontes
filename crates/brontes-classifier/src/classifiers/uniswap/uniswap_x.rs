use alloy_primitives::{Address, U256};
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
    logs: true,
    |
    info: CallInfo,
    logs_data: UniswapXExecuteCallLogs,
    _db_tx: &DB| {
        let fill_logs = logs_data.fill_field?;
        let solver = fill_logs[0].filler;

        let user_swaps = fill_logs.iter()
        .map(|fill| Fill::into_swap(fill, info.target_address))
        .collect();

        Ok(NormalizedBatch {
            protocol: Protocol::UniswapX,
            trace_index: info.trace_idx,
            solver,
            settlement_contract: info.target_address,
            user_swaps,
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
    |
    info: CallInfo,
    logs_data: UniswapXExecuteBatchCallLogs,
    _db_tx: &DB| {
        let fill_logs = logs_data.fill_field?;

        let solver = fill_logs[0].filler;

        let user_swaps = fill_logs.iter()
        .map(|fill| Fill::into_swap(fill, info.target_address))
        .collect();

        Ok(NormalizedBatch {
            protocol: Protocol::UniswapX,
            trace_index: info.trace_idx,
            solver,
            settlement_contract: info.target_address,
            user_swaps,
            solver_swaps: None,
            msg_value: info.msg_value,
        })
    }

);

action_impl!(
    Protocol::UniswapX,
    crate::UniswapX::executeBatchWithCallbackCall,
    Batch,
    [..Fill*],
    logs: true,
    |
    info: CallInfo,
    logs_data: UniswapXExecuteBatchWithCallbackCallLogs,
    _db_tx: &DB| {
        let fill_logs = logs_data.fill_field?;
        let solver = fill_logs[0].filler;

        let user_swaps = fill_logs.iter()
        .map(|fill| Fill::into_swap(fill, info.target_address))
        .collect::<Vec<_>>();

        Ok(NormalizedBatch {
            protocol: Protocol::UniswapX,
            trace_index: info.trace_idx,
            solver,
            settlement_contract: info.target_address,
            user_swaps,
            solver_swaps: None,
            msg_value: info.msg_value
        })
    }
);

action_impl!(
    Protocol::UniswapX,
    crate::UniswapX::executeWithCallbackCall,
    Batch,
    [..Fill*],
    logs: true,
    |
    info: CallInfo,
    logs_data: UniswapXExecuteWithCallbackCallLogs,
    _db_tx: &DB| {
        let fill_logs = logs_data.fill_field?;
        let solver = fill_logs[0].filler;

        let user_swaps = fill_logs.iter()
        .map(|fill| Fill::into_swap(fill, info.target_address))
        .collect();

        Ok(NormalizedBatch {
            protocol: Protocol::UniswapX,
            trace_index: info.trace_idx,
            solver,
            settlement_contract: info.target_address,
            user_swaps,
            solver_swaps: None,
            msg_value: info.msg_value
        })
    }
);

impl Fill {
    /// Here we're converting a Fill into a NormalizedSwap, however we don't yet
    /// have the full trade information.
    pub fn into_swap(fill_log: &Fill, settlement_contract: Address) -> NormalizedSwap {
        let swapper = fill_log.swapper;

        NormalizedSwap {
            protocol:    Protocol::UniswapX,
            trace_index: 0,
            from:        swapper,
            recipient:   swapper,
            pool:        settlement_contract,
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

    use alloy_primitives::{hex, B256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_pricing::Protocol::UniswapX;
    use brontes_types::{normalized_actions::Actions, ToScaledRational, TreeSearchBuilder};

    use super::*;

    #[brontes_macros::test]
    async fn test_batch_classifier_with_call_back_eth() {
        let classifier_utils = ClassifierTestUtils::new().await;
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
                    trace_index: 4,
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
                        6000da47483062A0D734Ba3dc7576Ce6A0B645C4"
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
                    trace_index: 7,
                    from:        Address::new(hex!(
                        "
                    569d9f244e4ed4f0731f39675492740dcdab6b15"
                    )),
                    recipient:   Address::new(hex!(
                        "
                    569d9f244e4ed4f0731f39675492740dcdab6b15"
                    )),
                    pool:        Address::new(hex!("6000da47483062A0D734Ba3dc7576Ce6A0B645C4")),
                    token_in:    TokenInfoWithAddress::usdt(),
                    amount_in:   U256::from_str("106496770").unwrap().to_scaled_rational(6),
                    token_out:   TokenInfoWithAddress::native_eth(),
                    amount_out:  U256::from_str("43925992451078510")
                        .unwrap()
                        .to_scaled_rational(18),
                    msg_value:   U256::ZERO,
                },
            ],
            solver_swaps:        None,
            msg_value:           U256::ZERO,
        });

        classifier_utils
            .contains_action(
                execute_batch_with_callback,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Actions::is_batch),
            )
            .await
            .unwrap();
    }

    #[brontes_macros::test]
    async fn test_batch_classifier_weth() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let execute_batch_with_callback =
            B256::from(hex!("f9e7365f9c9c2859effebe61d5d19f44dcbf4d2412e7bcc5c511b3b8fbfb8b8d"));

        let eq_action = Actions::Batch(NormalizedBatch {
            protocol:            Protocol::UniswapX,
            trace_index:         0,
            solver:              Address::new(hex!("ff8Ba4D1fC3762f6154cc942CCF30049A2A0cEC6")),
            settlement_contract: Address::new(hex!("6000da47483062A0D734Ba3dc7576Ce6A0B645C4")),
            user_swaps:          vec![NormalizedSwap {
                protocol:    UniswapX,
                trace_index: 3,
                from:        Address::new(hex!(
                    "
                        92069F3B51FF505e519378ba8229E3D1f51d472a"
                )),
                recipient:   Address::new(hex!(
                    "
                        92069F3B51FF505e519378ba8229E3D1f51d472a"
                )),
                pool:        Address::new(hex!(
                    "
                        6000da47483062A0D734Ba3dc7576Ce6A0B645C4"
                )),
                token_in:    TokenInfoWithAddress::weth(),
                amount_in:   U256::from_str("490000000000000000")
                    .unwrap()
                    .to_scaled_rational(18),
                token_out:   TokenInfoWithAddress::usdt(),
                amount_out:  U256::from_str("1182060728").unwrap().to_scaled_rational(6),

                msg_value: U256::ZERO,
            }],
            solver_swaps:        None,
            msg_value:           U256::ZERO,
        });

        classifier_utils
            .contains_action(
                execute_batch_with_callback,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Actions::is_batch),
            )
            .await
            .unwrap();
    }
}
