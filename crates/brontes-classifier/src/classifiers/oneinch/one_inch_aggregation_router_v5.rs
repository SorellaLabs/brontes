use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{normalized_actions::NormalizedAggregator, structured_trace::CallInfo};

action_impl!(
    Protocol::OneInchV5,
    crate::OneInchAggregationRouterV5::swapCall,
    Aggregator,
    [],
    call_data: true,
    |
    info: CallInfo,
    call_data: swapCall,
    _db_tx: &DB | {
        let dst_receiver = call_data.desc.dstReceiver;
        return Ok(NormalizedAggregator {
            protocol: Protocol::OneInchV5,
            trace_index: info.trace_idx,
            from: info.from_address,
            to: info.target_address
                , recipient: dst_receiver,
            child_actions: vec![],
            msg_value: info.msg_value
        })
    }
);

action_impl!(
    Protocol::OneInchV5,
    crate::OneInchAggregationRouterV5::fillOrderToCall,
    Aggregator,
    [..OrderFilled],
    call_data: true,
    |
    info: CallInfo,
    call_data: fillOrderToCall,
    _db_tx: &DB | {
        let recipient = call_data.order_.receiver;
        return Ok(NormalizedAggregator {
            protocol: Protocol::OneInchV5,
            trace_index: info.trace_idx,
            from: info.from_address,
            to: info.target_address,
            recipient,
            child_actions: vec![],
            msg_value: info.msg_value
        })
    }
);

action_impl!(
    Protocol::OneInchV5,
    crate::OneInchAggregationRouterV5::clipperSwapCall,
    Aggregator,
    [],
    |info: CallInfo, _db_tx: &DB| {
        return Ok(NormalizedAggregator {
            protocol:      Protocol::OneInchV5,
            trace_index:   info.trace_idx,
            from:          info.from_address,
            to:            info.target_address,
            recipient:     info.msg_sender,
            child_actions: vec![],
            msg_value:     info.msg_value,
        });
    }
);

action_impl!(
    Protocol::OneInchV5,
    crate::OneInchAggregationRouterV5::clipperSwapToCall,
    Aggregator,
    [],
    call_data: true,
    |
    info: CallInfo,
    call_data: clipperSwapToCall,
    _db_tx: &DB | {
        let recipient = call_data.recipient;
        return Ok(NormalizedAggregator {
            protocol: Protocol::OneInchV5,
            trace_index: info.trace_idx,
            from: info.from_address,
            to: info.target_address,
            recipient,
            child_actions: vec![],
            msg_value: info.msg_value
        })
    }
);

action_impl!(
    Protocol::OneInchV5,
    crate::OneInchAggregationRouterV5::unoswapToCall,
    Aggregator,
    [],
    call_data: true,
    |
    info: CallInfo,
    call_data: unoswapToCall,
    _db_tx: &DB | {
        let recipient = call_data.recipient;
        return Ok(NormalizedAggregator {
            protocol: Protocol::OneInchV5,
            trace_index: info.trace_idx,
            from: info.from_address,
            to: info.target_address,
            recipient,
            child_actions: vec![],
            msg_value: info.msg_value
        })
    }
);

action_impl!(
    Protocol::OneInchV5,
    crate::OneInchAggregationRouterV5::unoswapToWithPermitCall,
    Aggregator,
    [],
    call_data: true,
    |
    info: CallInfo,
    call_data: unoswapToWithPermitCall,
    _db_tx: &DB | {
        let recipient = call_data.recipient;
        return Ok(NormalizedAggregator {
            protocol: Protocol::OneInchV5,
            trace_index: info.trace_idx,
            from: info.from_address,
            to: info.target_address,
            recipient,
            child_actions: vec![],
            msg_value: info.msg_value
        })
    }
);

action_impl!(
    Protocol::OneInchV5,
    crate::OneInchAggregationRouterV5::uniswapV3SwapToWithPermitCall,
    Aggregator,
    [],
    call_data: true,
    |
    info: CallInfo,
    call_data: uniswapV3SwapToWithPermitCall,
    _db_tx: &DB | {
        let recipient = call_data.recipient;
        return Ok(NormalizedAggregator {
            protocol: Protocol::OneInchV5,
            trace_index: info.trace_idx,
            from: info.from_address,
            to: info.target_address,
            recipient,
            child_actions: vec![],
            msg_value: info.msg_value
        })
    }
);

action_impl!(
    Protocol::OneInchV5,
    crate::OneInchAggregationRouterV5::uniswapV3SwapToCall,
    Aggregator,
    [],
    call_data: true,
    |
    info: CallInfo,
    call_data: uniswapV3SwapToCall,
    _db_tx: &DB | {
        let recipient = call_data.recipient;
        return Ok(NormalizedAggregator {
            protocol: Protocol::OneInchV5,
            trace_index: info.trace_idx,
            from: info.from_address,
            to: info.target_address,
            recipient,
            child_actions: vec![],
            msg_value: info.msg_value
        })
    }
);

action_impl!(
    Protocol::OneInchV5,
    crate::OneInchAggregationRouterV5::clipperSwapToWithPermitCall,
    Aggregator,
    [],
    call_data: true,
    |
    info: CallInfo,
    call_data: clipperSwapToWithPermitCall,
    _db_tx: &DB | {
        let recipient = call_data.recipient;
        return Ok(NormalizedAggregator {
            protocol: Protocol::OneInchV5,
            trace_index: info.trace_idx,
            from: info.from_address,
            to: info.target_address,
            recipient,
            child_actions: vec![],
            msg_value: info.msg_value
        })
    }
);

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{hex, Address, B256, U256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_pricing::Protocol::UniswapV3;
    use brontes_types::{
        db::token_info::TokenInfoWithAddress,
        normalized_actions::{Action, NormalizedSwap, NormalizedTransfer},
        Protocol::OneInchV5,
        ToScaledRational, TreeSearchBuilder,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_one_inch_aggregator_swap() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let aggregator =
            B256::from(hex!("68603b7dce39738bc7aa9ce1cce39992965820ae39388a6d62db8d2db70132bb"));

        let eq_action = Action::Aggregator(NormalizedAggregator {
            protocol:      OneInchV5,
            trace_index:   0,
            from:          Address::new(hex!("f4F8845ceDe63e79De1B2c3bbA395e8547FE4283")),
            to:            Address::new(hex!("1111111254eeb25477b68fb85ed929f73a960582")),
            recipient:     Address::new(hex!("f4F8845ceDe63e79De1B2c3bbA395e8547FE4283")),
            child_actions: vec![
                Action::Transfer(NormalizedTransfer {
                    trace_index: 1,
                    msg_value:   U256::ZERO,
                    from:        Address::new(hex!("f4f8845cede63e79de1b2c3bba395e8547fe4283")),
                    to:          Address::new(hex!("e37e799d5077682fa0a244d46e5649f71457bd09")),
                    token:       TokenInfoWithAddress::usdc(),
                    amount:      U256::from_str("126000000000")
                        .unwrap()
                        .to_scaled_rational(6),
                    fee:         U256::from_str("0").unwrap().to_scaled_rational(1),
                }),
                Action::Transfer(NormalizedTransfer {
                    trace_index: 5,
                    msg_value:   U256::ZERO,
                    from:        Address::new(hex!("e37e799d5077682fa0a244d46e5649f71457bd09")),
                    to:          Address::new(hex!("beec796a4a2a27b687e1d48efad3805d78800522")),
                    token:       TokenInfoWithAddress::usdc(),
                    amount:      U256::from_str("441000000").unwrap().to_scaled_rational(6),
                    fee:         U256::from_str("0").unwrap().to_scaled_rational(1),
                }),
                Action::Swap(NormalizedSwap {
                    protocol:    UniswapV3,
                    trace_index: 11,
                    from:        Address::new(hex!("e37e799d5077682fa0a244d46e5649f71457bd09")),
                    recipient:   Address::new(hex!("e37e799d5077682fa0a244d46e5649f71457bd09")),
                    pool:        Address::new(hex!("3416cf6c708da44db2624d63ea0aaef7113527c6")),
                    token_in:    TokenInfoWithAddress::usdc(),
                    token_out:   TokenInfoWithAddress::usdt(),
                    amount_in:   U256::from_str("125559000000")
                        .unwrap()
                        .to_scaled_rational(6),
                    amount_out:  U256::from_str("125475168379")
                        .unwrap()
                        .to_scaled_rational(6),
                    msg_value:   U256::ZERO,
                }),
                Action::Transfer(NormalizedTransfer {
                    trace_index: 12,
                    msg_value:   U256::ZERO,
                    from:        Address::new(hex!("3416cf6c708da44db2624d63ea0aaef7113527c6")),
                    to:          Address::new(hex!("e37e799d5077682fa0a244d46e5649f71457bd09")),
                    token:       TokenInfoWithAddress::usdt(),
                    amount:      U256::from_str("125475168379")
                        .unwrap()
                        .to_scaled_rational(6),
                    fee:         U256::from_str("0").unwrap().to_scaled_rational(1),
                }),
                Action::Transfer(NormalizedTransfer {
                    trace_index: 16,
                    msg_value:   U256::ZERO,
                    from:        Address::new(hex!("e37e799d5077682fa0a244d46e5649f71457bd09")),
                    to:          Address::new(hex!("3416cf6c708da44db2624d63ea0aaef7113527c6")),
                    token:       TokenInfoWithAddress::usdc(),
                    amount:      U256::from_str("125559000000")
                        .unwrap()
                        .to_scaled_rational(6),
                    fee:         U256::from_str("0").unwrap().to_scaled_rational(1),
                }),
                Action::Transfer(NormalizedTransfer {
                    trace_index: 21,
                    msg_value:   U256::ZERO,
                    from:        Address::new(hex!("e37e799d5077682fa0a244d46e5649f71457bd09")),
                    to:          Address::new(hex!("1111111254eeb25477b68fb85ed929f73a960582")),
                    token:       TokenInfoWithAddress::usdt(),
                    amount:      U256::from_str("125475168379")
                        .unwrap()
                        .to_scaled_rational(6),
                    fee:         U256::from_str("0").unwrap().to_scaled_rational(1),
                }),
                Action::Transfer(NormalizedTransfer {
                    trace_index: 23,
                    msg_value:   U256::ZERO,
                    from:        Address::new(hex!("1111111254eeb25477b68fb85ed929f73a960582")),
                    to:          Address::new(hex!("f4f8845cede63e79de1b2c3bba395e8547fe4283")),
                    token:       TokenInfoWithAddress::usdt(),
                    amount:      U256::from_str("125475168379")
                        .unwrap()
                        .to_scaled_rational(6),
                    fee:         U256::from_str("0").unwrap().to_scaled_rational(1),
                }),
            ],

            msg_value: U256::ZERO,
        });

        classifier_utils
            .contains_action(
                aggregator,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Action::is_aggregator),
            )
            .await
            .unwrap();
    }
}
