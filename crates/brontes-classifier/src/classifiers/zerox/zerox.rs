use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::{NormalizedAggregator, NormalizedSwap, NormalizedBatch},
    structured_trace::CallInfo, ToScaledRational,
};
use alloy_primitives::U256;


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


action_impl!(
    Protocol::ZeroX,
    crate::ZeroXUniswapV3Feature::sellEthForTokenToUniswapV3Call,
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

action_impl!(
    Protocol::ZeroX,
    crate::ZeroXUniswapV3Feature::sellTokenForEthToUniswapV3Call,
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

action_impl!(
    Protocol::ZeroX,
    crate::ZeroXUniswapV3Feature::sellTokenForTokenToUniswapV3Call,
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

action_impl!(
    Protocol::ZeroX,
    crate::ZeroXOtcOrdersFeature::fillOtcOrderCall,
    Swap,
    [OtcOrderFilled],
    logs: true,
    |info: CallInfo, logs: ZeroXFillOtcOrderCallLogs, db: &DB| {
        let logs = logs.otc_order_filled_field?;

        let token_in = db.try_fetch_token_info(logs.makerToken)?;
        let token_out = db.try_fetch_token_info(logs.takerToken)?;

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
    crate::ZeroXOtcOrdersFeature::fillOtcOrderForEthCall,
    Swap,
    [OtcOrderFilled],
    logs: true,
    |info: CallInfo, logs: ZeroXFillOtcOrderForEthCallLogs, db: &DB| {
        let logs = logs.otc_order_filled_field?;

        let token_in = db.try_fetch_token_info(logs.makerToken)?;
        let token_out = db.try_fetch_token_info(logs.takerToken)?;

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
    crate::ZeroXOtcOrdersFeature::fillOtcOrderWithEthCall,
    Swap,
    [OtcOrderFilled],
    logs: true,
    |info: CallInfo, logs: ZeroXFillOtcOrderWithEthCallLogs, db: &DB| {
        let logs = logs.otc_order_filled_field?;

        let token_in = db.try_fetch_token_info(logs.makerToken)?;
        let token_out = db.try_fetch_token_info(logs.takerToken)?;

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
    crate::ZeroXOtcOrdersFeature::fillTakerSignedOtcOrderCall,
    Swap,
    [OtcOrderFilled],
    logs: true,
    |info: CallInfo, logs: ZeroXFillTakerSignedOtcOrderCallLogs, db: &DB| {
        let logs = logs.otc_order_filled_field?;

        let token_in = db.try_fetch_token_info(logs.makerToken)?;
        let token_out = db.try_fetch_token_info(logs.takerToken)?;

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

//fillTakerSignedOtcOrderForEth
action_impl!(
    Protocol::ZeroX,
    crate::ZeroXOtcOrdersFeature::fillTakerSignedOtcOrderForEthCall,
    Swap,
    [OtcOrderFilled],
    logs: true,
    |info: CallInfo, logs: ZeroXFillTakerSignedOtcOrderForEthCallLogs, db: &DB| {
        let logs = logs.otc_order_filled_field?;

        let token_in = db.try_fetch_token_info(logs.makerToken)?;
        let token_out = db.try_fetch_token_info(logs.takerToken)?;

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

//batchFillTakerSignedOtcOrders
//NormalizedBatch
//OtcOrderFilled*
action_impl!(
    Protocol::ZeroX,
    crate::ZeroXOtcOrdersFeature::batchFillTakerSignedOtcOrdersCall,
    Batch,
    [..OtcOrderFilled*],
    logs: true,
    |info: CallInfo, logs: ZeroXBatchFillTakerSignedOtcOrdersCallLogs, db: &DB| {
        let logs = logs.otc_order_filled_field?;

        let mut user_swaps = vec![];
        for log in logs {
            let token_in = db.try_fetch_token_info(log.makerToken)?;
            let token_out = db.try_fetch_token_info(log.takerToken)?;

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

// action_impl!(


// );