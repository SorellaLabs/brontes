use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::NormalizedAggregator, structured_trace::CallInfo, utils::ToScaledRational,
};


action_impl!(
    Protocol::OneInch,
    crate::OneInchFusionSettlement::settleOrdersCall,
    Aggregator,
    [],
    call_data: true,
    return_data: true,
    |
    info: CallInfo,
    call_data: swapCall,
    return_data: swapReturn,
    db_tx: &DB | {
        let dst_receiver = call_data.desc.dstReceiver;
        let token_in_amount = return_data.spentAmount;
        let token_out_amount = return_data.returnAmount;
        let token_in = db_tx.try_fetch_token_info(call_data.desc.srcToken)?;
        let token_out = db_tx.try_fetch_token_info(call_data.desc.dstToken)?;
        let amount_in = token_in_amount.to_scaled_rational(token_in.decimals);
        let amount_out = token_out_amount.to_scaled_rational(token_out.decimals);
        return Ok(NormalizedAggregator {
            protocol: Protocol::OneInch,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: dst_receiver,
            pool: info.target_address,
            token_in,
            token_out,
            amount_in,
            amount_out,
            child_actions: vec![],
            msg_value: info.msg_value
        })
    }
);