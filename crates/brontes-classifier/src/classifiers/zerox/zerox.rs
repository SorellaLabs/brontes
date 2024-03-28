use std::error::Error;

use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::NormalizedAggregator,
    structured_trace::CallInfo,
};


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

