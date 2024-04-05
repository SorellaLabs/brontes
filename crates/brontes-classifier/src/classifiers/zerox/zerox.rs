use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::{NormalizedAggregator, NormalizedSwap, NormalizedBatch},
    structured_trace::CallInfo, ToScaledRational,
};
use alloy_primitives::U256;

// Uniswap
action_impl!(
    Protocol::ZeroX,
    crate::ZeroXUniswapFeaure::sellToUniswapCall,
    Aggregator,
    [Swap],
    |info: CallInfo, _| {
        Ok(NormalizedAggregator {
            protocol: Protocol::ZeroX, 
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: info.msg_sender,
            child_actions: vec![],
            msg_value: info.msg_value, 
        })
    }
);

// Uniswap V3
action_impl!(
    Protocol::ZeroX,
    crate::ZeroXUniswapV3Feature::sellEthForTokenToUniswapV3Call,
    Aggregator,
    [Swap],
    call_data: true,
    |info: CallInfo, call_data: sellEthForTokenToUniswapV3Call, _| {
        Ok(NormalizedAggregator {
            protocol: Protocol::ZeroX, 
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: call_data.recipient,
            child_actions: vec![],
            msg_value: info.msg_value, 
        })
    }
);

action_impl!(
    Protocol::ZeroX,
    crate::ZeroXUniswapV3Feature::sellTokenForEthToUniswapV3Call,
    Aggregator,
    [Swap],
    call_data: true,
    |info: CallInfo, call_data: sellTokenForEthToUniswapV3Call, _| {
        Ok(NormalizedAggregator {
            protocol: Protocol::ZeroX, 
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: call_data.recipient,
            child_actions: vec![],
            msg_value: info.msg_value, 
        })
    }
);

action_impl!(
    Protocol::ZeroX,
    crate::ZeroXUniswapV3Feature::sellTokenForTokenToUniswapV3Call,
    Aggregator,
    [Swap],
    call_data: true,
    |info: CallInfo, call_data: sellTokenForTokenToUniswapV3Call, _| {
        Ok(NormalizedAggregator {
            protocol: Protocol::ZeroX, 
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: call_data.recipient,
            child_actions: vec![],
            msg_value: info.msg_value, 
        })
    }
);

// Transform
action_impl!(
    Protocol::ZeroX,
    crate::ZeroXTransformERC20Feature::transformERC20Call,
    Aggregator,
    [Swap],
    |info: CallInfo, _| {
        Ok(NormalizedAggregator {
            protocol: Protocol::ZeroX, 
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: info.msg_sender,
            child_actions: vec![],
            msg_value: info.msg_value, 
        })
    }
);

// Pancakeswap
action_impl!(
    Protocol::ZeroX,
    crate::ZeroXPancakeSwapFeature::sellToPancakeSwapCall,
    Aggregator,
    [Swap],
    |info: CallInfo, _| {
        Ok(NormalizedAggregator {
            protocol: Protocol::ZeroX, 
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: info.msg_sender,
            child_actions: vec![],
            msg_value: info.msg_value, 
        })
    }
);

// Otc orders
action_impl!(
    Protocol::ZeroX,
    crate::ZeroXOtcOrdersFeature::fillOtcOrderCall,
    Swap,
    [OtcOrderFilled],
    logs: true,
    include_delegated_logs: true,
    |info: CallInfo, logs: ZeroXFillOtcOrderCallLogs, db: &DB| {
        let logs = logs.otc_order_filled_field?;

        let token_in = db.try_fetch_token_info(logs.takerToken)?;
        let token_out = db.try_fetch_token_info(logs.makerToken)?;

        let amount_in = U256::from(logs.takerTokenFilledAmount).to_scaled_rational(token_in.decimals);
        let amount_out = U256::from(logs.makerTokenFilledAmount).to_scaled_rational(token_out.decimals);

        Ok(NormalizedSwap {
            protocol: Protocol::ZeroX,
            trace_index: info.trace_idx,
            from: logs.taker,
            recipient: logs.taker,
            msg_value :info.msg_value, 
            pool: info.target_address, 
            token_in, 
            token_out, 
            amount_in, 
            amount_out 
        })
    }
);

action_impl!(
    Protocol::ZeroX,
    crate::ZeroXOtcOrdersFeature::fillOtcOrderForEthCall,
    Swap,
    [OtcOrderFilled],
    logs: true,
    include_delegated_logs: true,
    |info: CallInfo, logs: ZeroXFillOtcOrderForEthCallLogs, db: &DB| {
        let logs = logs.otc_order_filled_field?;

        let token_in = db.try_fetch_token_info(logs.takerToken)?;
        let token_out = db.try_fetch_token_info(logs.makerToken)?;

        let amount_in = U256::from(logs.takerTokenFilledAmount).to_scaled_rational(token_in.decimals);
        let amount_out = U256::from(logs.makerTokenFilledAmount).to_scaled_rational(token_out.decimals);

        Ok(NormalizedSwap {
            protocol: Protocol::ZeroX,
            trace_index: info.trace_idx,
            from: logs.taker,
            recipient: logs.taker,
            msg_value :info.msg_value, 
            pool: info.target_address, 
            token_in, 
            token_out, 
            amount_in, 
            amount_out 
        })
    }
);

action_impl!(
    Protocol::ZeroX,
    crate::ZeroXOtcOrdersFeature::fillOtcOrderWithEthCall,
    Swap,
    [OtcOrderFilled],
    logs: true,
    include_delegated_logs: true,
    |info: CallInfo, logs: ZeroXFillOtcOrderWithEthCallLogs, db: &DB| {
        let logs = logs.otc_order_filled_field?;

        let token_in = db.try_fetch_token_info(logs.takerToken)?;
        let token_out = db.try_fetch_token_info(logs.makerToken)?;

        let amount_in = U256::from(logs.takerTokenFilledAmount).to_scaled_rational(token_in.decimals);
        let amount_out = U256::from(logs.makerTokenFilledAmount).to_scaled_rational(token_out.decimals);

        Ok(NormalizedSwap {
            protocol: Protocol::ZeroX,
            trace_index: info.trace_idx,
            from: logs.taker,
            recipient: logs.taker,
            msg_value :info.msg_value, 
            pool: info.target_address, 
            token_in, 
            token_out, 
            amount_in, 
            amount_out 
        })
    }
);

//https://etherscan.io/tx/0x92ea4576989a38d630867ff361c346d9317e2f61a3192a0c03698d9a70b5aee2
action_impl!(
    Protocol::ZeroX,
    crate::ZeroXOtcOrdersFeature::fillTakerSignedOtcOrderCall,
    Swap,
    [OtcOrderFilled],
    logs: true,
    include_delegated_logs: true,
    |info: CallInfo, logs: ZeroXFillTakerSignedOtcOrderCallLogs, db: &DB| {
        let logs = logs.otc_order_filled_field?;

        let token_in = db.try_fetch_token_info(logs.takerToken)?;
        let token_out = db.try_fetch_token_info(logs.makerToken)?;

        let amount_in = U256::from(logs.takerTokenFilledAmount).to_scaled_rational(token_in.decimals);
        let amount_out = U256::from(logs.makerTokenFilledAmount).to_scaled_rational(token_out.decimals);

        Ok(NormalizedSwap {
            protocol: Protocol::ZeroX,
            trace_index: info.trace_idx,
            from: logs.taker,
            recipient: logs.taker,
            msg_value :info.msg_value, 
            pool: info.target_address, 
            token_in, 
            token_out, 
            amount_in, 
            amount_out 
        })
    }
);

//https://etherscan.io/tx/0x2ba6ce2e47a4625b75a64bd0a22b4b288ffd7582cd2ac559962e456e6bb7fe61
action_impl!(
    Protocol::ZeroX,
    crate::ZeroXOtcOrdersFeature::fillTakerSignedOtcOrderForEthCall,
    Swap,
    [OtcOrderFilled],
    logs: true,
    |info: CallInfo, logs: ZeroXFillTakerSignedOtcOrderForEthCallLogs, db: &DB| {
        let logs = logs.otc_order_filled_field?;

        let token_in = db.try_fetch_token_info(logs.takerToken)?;
        let token_out = db.try_fetch_token_info(logs.makerToken)?;

        let amount_in = U256::from(logs.makerTokenFilledAmount).to_scaled_rational(token_in.decimals);
        let amount_out = U256::from(logs.takerTokenFilledAmount).to_scaled_rational(token_out.decimals);

        Ok(NormalizedSwap {
            protocol: Protocol::ZeroX,
            trace_index: info.trace_idx,
            from: logs.maker,
            recipient: logs.taker,
            msg_value :info.msg_value, 
            pool: info.target_address, 
            token_in, 
            token_out, 
            amount_in, 
            amount_out 
        })
    }
);

action_impl!(
    Protocol::ZeroX,
    crate::ZeroXOtcOrdersFeature::batchFillTakerSignedOtcOrdersCall,
    Batch,
    [..OtcOrderFilled*],
    logs: true,
    include_delegated_logs: true,
    |info: CallInfo, logs: ZeroXBatchFillTakerSignedOtcOrdersCallLogs, db: &DB| {
        let logs = logs.otc_order_filled_field?;

        let mut user_swaps = vec![];
        for log in logs {
            let token_in = db.try_fetch_token_info(log.takerToken)?;
            let token_out = db.try_fetch_token_info(log.makerToken)?;

            let amount_in = U256::from(log.makerTokenFilledAmount).to_scaled_rational(token_in.decimals);
            let amount_out = U256::from(log.takerTokenFilledAmount).to_scaled_rational(token_out.decimals);

            user_swaps.push(NormalizedSwap {
                protocol: Protocol::ZeroX,
                trace_index: info.trace_idx,
                from: log.maker,
                recipient: log.taker,
                msg_value :info.msg_value, 
                pool: info.target_address, 
                token_in, 
                token_out, 
                amount_in, 
                amount_out 
            });
        }

        Ok(NormalizedBatch {
            protocol: Protocol::ZeroX,
            trace_index: info.trace_idx,
            solver: info.from_address,
            settlement_contract: info.target_address,
            solver_swaps: None,
            user_swaps,
            msg_value: info.msg_value, 
        })
    }
);

// Liquidity Provider
//https://etherscan.io/tx/0x58b26d0fa1dcafd8af70e9adc8b9ca08dee9d2f63ae9e7a5430830c160ca0ceb
action_impl!(
    Protocol::ZeroX,
    crate::ZeroXLiquidityProviderFeature::sellToLiquidityProviderCall,
    Swap,
    [LiquidityProviderSwap],
    logs: true,
    |info: CallInfo, logs: ZeroXSellToLiquidityProviderCallLogs, db: &DB| {
        let logs = logs.liquidity_provider_swap_field?;

        let token_in = db.try_fetch_token_info(logs.inputToken)?;
        let token_out = db.try_fetch_token_info(logs.outputToken)?;

        let amount_in = U256::from(logs.inputTokenAmount).to_scaled_rational(token_in.decimals);
        let amount_out = U256::from(logs.outputTokenAmount).to_scaled_rational(token_out.decimals);

        Ok(NormalizedSwap {
            protocol: Protocol::ZeroX,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: logs.recipient,
            msg_value :info.msg_value, 
            pool: info.target_address, 
            token_in, 
            token_out, 
            amount_in, 
            amount_out 
        })
    }

);

// Multiplex
//https://etherscan.io/tx/0xff79232fe5aca01c6f5d85ed5f14bd10ca5f58584c4f6707fa5910e2eda79262
action_impl!(
    Protocol::ZeroX,
    crate::ZeroXInterface::multiplexBatchSellEthForTokenCall,
    Aggregator,
    [],
    |info: CallInfo, _| {

        Ok(NormalizedAggregator {
            protocol: Protocol::ZeroX, 
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: info.msg_sender,
            child_actions: vec![],
            msg_value: info.msg_value, 
        })
    }
);

action_impl!(
    Protocol::ZeroX,
    crate::ZeroXInterface::multiplexBatchSellTokenForEthCall,
    Aggregator,
    [],
    |info: CallInfo, _| {

        Ok(NormalizedAggregator {
            protocol: Protocol::ZeroX, 
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: info.msg_sender,
            child_actions: vec![],
            msg_value: info.msg_value, 
        })
    }
);

action_impl!(
    Protocol::ZeroX,
    crate::ZeroXInterface::multiplexBatchSellTokenForTokenCall,
    Aggregator,
    [],
    |info: CallInfo, _| {

        Ok(NormalizedAggregator {
            protocol: Protocol::ZeroX, 
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: info.msg_sender,
            child_actions: vec![],
            msg_value: info.msg_value, 
        })
    }
);

action_impl!(
    Protocol::ZeroX,
    crate::ZeroXInterface::multiplexMultiHopSellEthForTokenCall,
    Aggregator,
    [],
    |info: CallInfo, _| {

        Ok(NormalizedAggregator {
            protocol: Protocol::ZeroX, 
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: info.msg_sender,
            child_actions: vec![],
            msg_value: info.msg_value, 
        })
    }
);

action_impl!(
    Protocol::ZeroX,
    crate::ZeroXInterface::multiplexMultiHopSellTokenForEthCall,
    Aggregator,
    [],
    |info: CallInfo, _| {

        Ok(NormalizedAggregator {
            protocol: Protocol::ZeroX, 
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: info.msg_sender,
            child_actions: vec![],
            msg_value: info.msg_value, 
        })
    }
);

action_impl!(
    Protocol::ZeroX,
    crate::ZeroXInterface::multiplexMultiHopSellTokenForTokenCall,
    Aggregator,
    [],
    |info: CallInfo, _| {

        Ok(NormalizedAggregator {
            protocol: Protocol::ZeroX, 
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: info.msg_sender,
            child_actions: vec![],
            msg_value: info.msg_value, 
        })
    }
);

// Native Orders
// TODO, CODE NOT IN GITHUB


#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{hex, Address, B256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        db::token_info::{TokenInfo, TokenInfoWithAddress},
        normalized_actions::Actions,
        TreeSearchBuilder,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_zerox_sell_to_uniswap() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let aggregator_tx =
            B256::from(hex!("fac5edf3af538243554fdb0d8275781ee5834686bc0881e9343ac90e108a9e89"));

        classifier_utils
            .detects_protocol_at(
                aggregator_tx,
                0,
                Protocol::ZeroX,
                TreeSearchBuilder::default().with_action(Actions::is_aggregator),
            )
            .await
            .unwrap();
    }

    #[brontes_macros::test]
    async fn test_zerox_sell_eth_for_token_to_uniswap_v3() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let aggregator_tx =
            B256::from(hex!("d168fb3a2f2bc931ba7974d6afa89e2843c251f9fad444b71033f2c7b1953c9e"));

        classifier_utils
            .detects_protocol_at(
                aggregator_tx,
                0,
                Protocol::ZeroX,
                TreeSearchBuilder::default().with_action(Actions::is_aggregator),
            )
            .await
            .unwrap();
    }

    #[brontes_macros::test]
    async fn test_zerox_sell_token_for_eth_to_uniswap_v3() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let aggregator_tx =
            B256::from(hex!("8c4f1512afc633047ea7bc71484265cadba410adb6de99981b2f5220748b5fc2"));

        classifier_utils
            .detects_protocol_at(
                aggregator_tx,
                0,
                Protocol::ZeroX,
                TreeSearchBuilder::default().with_action(Actions::is_aggregator),
            )
            .await
            .unwrap();
    }

    #[brontes_macros::test]
    async fn test_zerox_sell_token_for_token_to_uniswap_v3() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let aggregator_tx =
            B256::from(hex!("aa8f632e139d59dc67f58ea2d9faee6f076eae08098ba08de24658b56fa09cfe"));

        classifier_utils
            .detects_protocol_at(
                aggregator_tx,
                0,
                Protocol::ZeroX,
                TreeSearchBuilder::default().with_action(Actions::is_aggregator),
            )
            .await
            .unwrap();
    }

    #[brontes_macros::test]
    async fn test_zerox_transform_erc20() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let aggregator_tx =
            B256::from(hex!("cd3cb6d905be10df9e1ad080eed2e8253af7a46aec27f64607b0145c9051e838"));

        classifier_utils
            .detects_protocol_at(
                aggregator_tx,
                0,
                Protocol::ZeroX,
                TreeSearchBuilder::default().with_action(Actions::is_aggregator),
            )
            .await
            .unwrap();
    }

    #[brontes_macros::test]
    async fn test_zerox_fill_otc_order() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let swap_tx =
            B256::from(hex!("07a010a8697a5d74c1c68dac628e18f5b09e593dc89f6a7d11b2bf7873dad726"));

        let token_in = TokenInfoWithAddress::weth();
        let token_out = TokenInfoWithAddress {
            address: Address::from_str("0xae7ab96520DE3A18E5e111B5EaAb095312D7fE84").unwrap(),
            inner: TokenInfo {
                decimals: 18,
                symbol: "stETH".to_string()
            }
        };

        classifier_utils.ensure_token(token_in.clone());

        let amount_in = U256::from_str("4127334728116880329")
            .unwrap()
            .to_scaled_rational(token_in.decimals);
        let amount_out = U256::from_str("4123654490488176400")
            .unwrap()
            .to_scaled_rational(token_out.decimals);


        let action = Actions::Swap(NormalizedSwap {
            protocol: Protocol::ZeroX,
            trace_index: 0,
            from: Address::from_str("0x69Db96B584B6e25420a4Aa2ca4B20E3860d19d8C").unwrap(),
            recipient: Address::from_str("0x69Db96B584B6e25420a4Aa2ca4B20E3860d19d8C").unwrap(),
            msg_value: U256::ZERO,
            pool: Address::from_str("0xdef1c0ded9bec7f1a1670819833240f027b25eff").unwrap(),
            token_in,
            token_out,
            amount_in,
            amount_out,
        });

        classifier_utils
            .contains_action(
                swap_tx, 
                0, 
                action,               
                TreeSearchBuilder::default().with_action(Actions::is_swap),
            )
            .await
            .unwrap();
    }

    #[brontes_macros::test]
    async fn test_zerox_fill_otc_order_for_eth() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let swap_tx =
            B256::from(hex!("b42a52833022a55565a1822c794f31b09612114fdca7b8445393547c0f45c900"));

        let token_in = TokenInfoWithAddress {
            address: Address::from_str("0xdac17f958d2ee523a2206206994597c13d831ec7").unwrap(),
            inner: TokenInfo {
                decimals: 6,
                symbol: "USDT".to_string()
            }
        };
        let token_out = TokenInfoWithAddress::weth();


        classifier_utils.ensure_token(token_out.clone());

        let amount_in = U256::from_str("100000000000")
            .unwrap()
            .to_scaled_rational(token_in.decimals);
        let amount_out = U256::from_str("41164546659018235904")
            .unwrap()
            .to_scaled_rational(token_out.decimals);

        let action = Actions::Swap(NormalizedSwap {
            protocol: Protocol::ZeroX,
            trace_index: 0,
            from: Address::from_str("0x84e8567695361adf883b6d2e12d22e9f0352bd06").unwrap(),
            recipient: Address::from_str("0x84e8567695361adf883b6d2e12d22e9f0352bd06").unwrap(),
            msg_value: U256::ZERO,
            pool: Address::from_str("0xdef1c0ded9bec7f1a1670819833240f027b25eff").unwrap(),
            token_in,
            token_out,
            amount_in,
            amount_out,
        });

        classifier_utils
            .contains_action(
                swap_tx, 
                0, 
                action,               
                TreeSearchBuilder::default().with_action(Actions::is_swap),
            )
            .await
            .unwrap();
    }

    #[brontes_macros::test]
    async fn test_zerox_fill_otc_order_with_eth() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let swap_tx =
            B256::from(hex!("9e9b85c90ed4bcb1a7579c048748a5c232685743bf945ec4b54399ca63268e48"));
        
            let token_in = TokenInfoWithAddress::weth();
            let token_out = TokenInfoWithAddress {
                address: Address::from_str("0xfAbA6f8e4a5E8Ab82F62fe7C39859FA577269BE3").unwrap(),
                inner: TokenInfo {
                    decimals: 18,
                    symbol: "ONDO".to_string()
                }
            };
    
            classifier_utils.ensure_token(token_out.clone());
    
            let amount_in = U256::from_str("247714108230076030")
                .unwrap()
                .to_scaled_rational(token_in.decimals);
            let amount_out = U256::from_str("2448668913450061000000")
                .unwrap()
                .to_scaled_rational(token_out.decimals);
    
            let action = Actions::Swap(NormalizedSwap {
                protocol: Protocol::ZeroX,
                trace_index: 0,
                from: Address::from_str("0xaaf46B4718e2251F682171a88bad79dAb3AcF35C").unwrap(),
                recipient: Address::from_str("0xaaf46B4718e2251F682171a88bad79dAb3AcF35C").unwrap(),
                msg_value: U256::from_str("247714108230076030").unwrap(),
                pool: Address::from_str("0xdef1c0ded9bec7f1a1670819833240f027b25eff").unwrap(),
                token_in,
                token_out,
                amount_in,
                amount_out,
            });
    
            classifier_utils
                .contains_action(
                    swap_tx, 
                    0, 
                    action,               
                    TreeSearchBuilder::default().with_action(Actions::is_swap),
                )
                .await
                .unwrap();
    }

    #[brontes_macros::test]
    async fn test_zerox_fill_taker_signed_otc_order() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let swap_tx =
            B256::from(hex!("92ea4576989a38d630867ff361c346d9317e2f61a3192a0c03698d9a70b5aee2"));

        let token_in = TokenInfoWithAddress {
            address: Address::from_str("0x6De037ef9aD2725EB40118Bb1702EBb27e4Aeb24").unwrap(),
            inner: TokenInfo {
                decimals: 18,
                symbol: "RNDR".to_string()
            }
        };
        let token_out = TokenInfoWithAddress {
            address: Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
            inner: TokenInfo {
                decimals: 6,
                symbol: "USDC".to_string()
            }
        };

        classifier_utils.ensure_token(token_in.clone());

        let amount_in = U256::from_str("224799926605806500000")
            .unwrap()
            .to_scaled_rational(token_in.decimals);
        let amount_out = U256::from_str("1000000000")
            .unwrap()
            .to_scaled_rational(token_out.decimals);

        let action = Actions::Swap(NormalizedSwap {
            protocol: Protocol::ZeroX,
            trace_index: 0,
            from: Address::from_str("0xCdaf004c23184aBa394A2d0476e7cEb33BA16C2c").unwrap(),
            recipient: Address::from_str("0xCdaf004c23184aBa394A2d0476e7cEb33BA16C2c").unwrap(),
            msg_value: U256::ZERO,
            pool: Address::from_str("0xdef1c0ded9bec7f1a1670819833240f027b25eff").unwrap(),
            token_in,
            token_out,
            amount_in,
            amount_out,
        });

        classifier_utils
            .contains_action(
                swap_tx, 
                0, 
                action,               
                TreeSearchBuilder::default().with_action(Actions::is_swap),
            )
            .await
            .unwrap()
    }
}