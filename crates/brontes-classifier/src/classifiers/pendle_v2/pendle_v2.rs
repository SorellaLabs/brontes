use brontes_macros::action_impl;
use brontes_types::{
    normalized_actions::{NormalizedBurn, NormalizedMint, NormalizedSwap},
    structured_trace::CallInfo,
    Protocol, ToScaledRational,
};

action_impl!(
    Protocol::PendleV2,
    crate::PendleSYToken::depositCall,
    Swap,
    [Deposit],
    call_data:true,
    return_data: true,
    |info: CallInfo, call_data:depositCall, return_data:depositReturn, db_tx: &DB| {
    let amount_underlying_in=call_data.amountTokenToDeposit;
    let amount_sy_out=return_data.amountSharesOut;

    let token_in = db_tx.try_fetch_token_info(call_data.tokenIn)?;
    let token_out = db_tx.try_fetch_token_info(info.target_address)?;

    let amount_in = amount_underlying_in.to_scaled_rational(token_in.decimals);
    let amount_out = amount_sy_out.to_scaled_rational(token_out.decimals);

    Ok(NormalizedSwap {
        protocol: Protocol::PendleV2,
        trace_index: info.trace_idx,
        from: info.from_address,
        recipient: call_data.receiver,
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
    Protocol::PendleV2,
    crate::PendleSYToken::redeemCall,
    Swap,
    [Redeem],
    call_data:true,
    return_data: true,
    |info: CallInfo, call_data:redeemCall, return_data:redeemReturn, db_tx: &DB| {
        let amount_sy_in=call_data.amountSharesToRedeem;
        let amount_underlying_out=return_data.amountTokenOut;

        let token_in = db_tx.try_fetch_token_info(info.target_address)?;
        let token_out = db_tx.try_fetch_token_info(call_data.tokenOut)?;

        let amount_in = amount_sy_in.to_scaled_rational(token_in.decimals);
        let amount_out = amount_underlying_out.to_scaled_rational(token_out.decimals);

        Ok(NormalizedSwap {
            protocol: Protocol::PendleV2,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: call_data.receiver,
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
    Protocol::PendleV2,
    crate::PendleMarketV3::swapExactPtForSyCall,
    Swap,
    [Swap],
    call_data:true,
    return_data: true,
    |info: CallInfo, call_data:swapExactPtForSyCall, return_data:swapExactPtForSyReturn, db_tx: &DB| {
    let amount_pt_in=call_data.exactPtIn;
    let amount_sy_out=return_data.netSyOut;

    let details=db_tx.get_protocol_details(info.target_address)?;

    let sy=details.token0;
    let pt=details.token1;

    let token_in = db_tx.try_fetch_token_info(pt)?;
    let token_out = db_tx.try_fetch_token_info(sy)?;

    let amount_in = amount_pt_in.to_scaled_rational(token_in.decimals);
    let amount_out = amount_sy_out.to_scaled_rational(token_out.decimals);

    Ok(NormalizedSwap {
        protocol: Protocol::PendleV2,
        trace_index: info.trace_idx,
        from: info.from_address,
        recipient: call_data.receiver,
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
    Protocol::PendleV2,
    crate::PendleMarketV3::swapSyForExactPtCall,
    Swap,
    [Swap],
    call_data:true,
    return_data: true,
    |info: CallInfo, call_data:swapSyForExactPtCall, return_data:swapSyForExactPtReturn, db_tx: &DB| {
    let amount_pt_out=call_data.exactPtOut;
    let amount_sy_in=return_data.netSyIn;

    let details=db_tx.get_protocol_details(info.target_address)?;

    let sy=details.token0;
    let pt=details.token1;

    let token_in = db_tx.try_fetch_token_info(sy)?;
    let token_out = db_tx.try_fetch_token_info(pt)?;

    let amount_in = amount_sy_in.to_scaled_rational(token_in.decimals);
    let amount_out = amount_pt_out.to_scaled_rational(token_out.decimals);

    Ok(NormalizedSwap {
        protocol: Protocol::PendleV2,
        trace_index: info.trace_idx,
        from: info.from_address,
        recipient: call_data.receiver,
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
    Protocol::PendleV2,
    crate::PendleMarketV3::mintCall,
    Mint,
    [Mint],
    call_data: true,
    return_data: true,
    |
     info: CallInfo,
     call_data: mintCall,
     return_data: mintReturn, db_tx: &DB| {
        let token_pt_delta=return_data.netPtUsed;
        let token_sy_delta=return_data.netSyUsed;

        let details=db_tx.get_protocol_details(info.target_address)?;
        let [token_sy, token_pt]=[details.token0, details.token1];

        let t0_info=db_tx.try_fetch_token_info(token_pt)?;
        let t1_info=db_tx.try_fetch_token_info(token_sy)?;

        let am0=token_pt_delta.to_scaled_rational(t0_info.decimals);
        let am1=token_sy_delta.to_scaled_rational(t1_info.decimals);
        Ok(NormalizedMint {
            protocol: Protocol::PendleV2,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: call_data.receiver,
            pool: info.target_address,
            token: vec![t0_info, t1_info],
            amount: vec![am0, am1],
        })
    }
);

action_impl!(
    Protocol::PendleV2,
    crate::PendleMarketV3::burnCall,
    Burn,
    [Burn],
    call_data: true,
    return_data: true,
    |
    info: CallInfo,
    call_data: burnCall,
    return_data: burnReturn,
    db_tx: &DB| {
        let token_pt_delta=return_data.netPtOut;
        let token_sy_delta=return_data.netSyOut;
        let details = db_tx.get_protocol_details(info.target_address)?;
        let [token_sy, token_pt] = [details.token0, details.token1];

        let t0_info = db_tx.try_fetch_token_info(token_pt)?;
        let t1_info = db_tx.try_fetch_token_info(token_sy)?;

        let am0 = token_pt_delta.to_scaled_rational(t0_info.decimals);
        let am1 = token_sy_delta.to_scaled_rational(t1_info.decimals);

        // assume the receiver is the same for Sy and Pt
        Ok(NormalizedBurn {
            protocol: Protocol::PendleV2,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: call_data.receiverSy,
            pool: info.target_address,
            token: vec![t0_info, t1_info],
            amount: vec![am0, am1],
        })
    }
);
