use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::NormalizedSwap, structured_trace::CallInfo, utils::ToScaledRational,
};
use crate::OneInchAggregationRouterV5::{swapReturn, clipperSwapReturn, clipperSwapToReturn, clipperSwapToWithPermitReturn, unoswapReturn};

action_impl!(
    Protocol::OneInch,
    crate::OneInchAggregationRouterV5::swapCall,
    Swap,
    [],
    call_data: true,
    return_data: true,
    |
    info: CallInfo,
    call_data: swapCall,
    return_data: swapReturn,
    db_tx: &DB | {
        let src_receiver = call_data.desc.srcReceiver;
        let dst_receiver = call_data.desc.dstReceiver;
        let token_in_amount = return_data.spentAmount;
        let token_out_amount = return_data.returnAmount;
        let token_in = db_tx.try_fetch_token_info(call_data.desc.srcToken)?;
        let token_out = db_tx.try_fetch_token_info(call_data.desc.dstToken)?;
        let amount_in = token_in_amount.to_scaled_rational(token_in.decimals);
        let amount_out = token_out_amount.to_scaled_rational(token_out.decimals);
        return Ok(NormalizedSwap {
            protocol: Protocol::OneInch,
            trace_index: info.trace_idx,
            from: src_receiver,
            recipient: dst_receiver,
            pool: info.target_address,
            token_in,
            token_out,
            amount_in,
            amount_out,
            msg_value: info.msg_value
        })
    }
);

action_impl!(
    Protocol::OneInch,
    crate::OneInchAggregationRouterV5::clipperSwapCall,
    Swap,
    [],
    call_data: true,
    return_data: true,
    |
    info: CallInfo,
    call_data: clipperSwapCall,
    return_data: clipperSwapReturn,
    db_tx: &DB | {
        let token_in_amount = call_data.inputAmount;
        let token_out_amount = return_data.returnAmount;
        let token_in = db_tx.try_fetch_token_info(call_data.srcToken)?;
        let token_out = db_tx.try_fetch_token_info(call_data.dstToken)?;
        let amount_in = token_in_amount.to_scaled_rational(token_in.decimals);
        let amount_out = token_out_amount.to_scaled_rational(token_out.decimals);
        return Ok(NormalizedSwap {
            protocol: Protocol::OneInch,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: info.msg_sender,
            pool: info.target_address,
            token_in,
            token_out,
            amount_in,
            amount_out,
            msg_value: info.msg_value
        })
    }
);

action_impl!(
    Protocol::OneInch,
    crate::OneInchAggregationRouterV5::clipperSwapToCall,
    Swap,
    [],
    call_data: true,
    return_data: true,
    |
    info: CallInfo,
    call_data: clipperSwapToCall,
    return_data: clipperSwapToReturn,
    db_tx: &DB | {
        let recipient = call_data.recipient;
        let token_in_amount = call_data.inputAmount;
        let token_out_amount = return_data.returnAmount;
        let token_in = db_tx.try_fetch_token_info(call_data.srcToken)?;
        let token_out = db_tx.try_fetch_token_info(call_data.dstToken)?;
        let amount_in = token_in_amount.to_scaled_rational(token_in.decimals);
        let amount_out = token_out_amount.to_scaled_rational(token_out.decimals);
        return Ok(NormalizedSwap {
            protocol: Protocol::OneInch,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient,
            pool: info.target_address,
            token_in,
            token_out,
            amount_in,
            amount_out,
            msg_value: info.msg_value
        })
    }
);

action_impl!(
    Protocol::OneInch,
    crate::OneInchAggregationRouterV5::clipperSwapToWithPermitCall,
    Swap,
    [],
    call_data: true,
    return_data: true,
    |
    info: CallInfo,
    call_data: clipperSwapToWithPermitCall,
    return_data: clipperSwapToWithPermitReturn,
    db_tx: &DB | {
        let recipient = call_data.recipient;
        let token_in_amount = call_data.inputAmount;
        let token_out_amount = return_data.returnAmount;
        let token_in = db_tx.try_fetch_token_info(call_data.srcToken)?;
        let token_out = db_tx.try_fetch_token_info(call_data.dstToken)?;
        let amount_in = token_in_amount.to_scaled_rational(token_in.decimals);
        let amount_out = token_out_amount.to_scaled_rational(token_out.decimals);
        return Ok(NormalizedSwap {
            protocol: Protocol::OneInch,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient,
            pool: info.target_address,
            token_in,
            token_out,
            amount_in,
            amount_out,
            msg_value: info.msg_value
        })
    }
);