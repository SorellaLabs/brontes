use alloy_primitives::Address;
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{normalized_actions::NormalizedSwap, ToScaledRational};

use crate::BalancerV1::{swapExactAmountInReturn, swapExactAmountOutReturn};

action_impl!(
    Protocol::BalancerV1,
    crate::BalancerV1::swapExactAmountInCall,
    Swap,
    [Swap],
    call_data: true,
    return_data: true,
    |trace_index,
    from_address: Address,
    target_address: Address,
    _msg_sender: Address,
    call_data: swapExactAmountInCall,
    return_data: swapExactAmountInReturn,
    db_tx: &DB| {
        let token_in = db_tx.try_get_token_info(call_data.tokenIn).ok()??;
        let token_out = db_tx.try_get_token_info(call_data.tokenOut).ok()??;
        let amount_in = call_data.tokenAmountIn.to_scaled_rational(token_in.decimals);
        let amount_out = return_data.tokenAmountOut.to_scaled_rational(token_out.decimals);

        Some(NormalizedSwap {
            protocol: Protocol::BalancerV1,
            trace_index,
            from: from_address,
            recipient: _msg_sender,
            pool: target_address,
            token_in,
            token_out,
            amount_in,
            amount_out,
        })
    }
);

action_impl!(
    Protocol::BalancerV1,
    crate::BalancerV1::swapExactAmountOutCall,
    Swap,
    [Swap],
    call_data: true,
    return_data: true,
    |trace_index,
    from_address: Address,
    target_address: Address,
    _msg_sender: Address,
    call_data: swapExactAmountOutCall,
    return_data: swapExactAmountOutReturn,
    db_tx: &DB| {
        let token_in = db_tx.try_get_token_info(call_data.tokenIn).ok()??;
        let token_out = db_tx.try_get_token_info(call_data.tokenOut).ok()??;
        let amount_in = return_data.tokenAmountIn.to_scaled_rational(token_in.decimals);
        let amount_out = call_data.tokenAmountOut.to_scaled_rational(token_out.decimals);


        Some(NormalizedSwap {
            protocol: Protocol::BalancerV1,
            trace_index,
            from: from_address,
            recipient: _msg_sender,
            pool: target_address,
            token_in,
            token_out,
            amount_in,
            amount_out,
        })
    }
);
