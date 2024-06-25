use alloy_primitives::U256;
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::{NormalizedBurn, NormalizedMint, NormalizedSwap},
    structured_trace::CallInfo,
    ToScaledRational,
};

action_impl!(
    Protocol::UniswapV2,
    crate::UniswapV2::swapCall,
    Swap,
    [..Swap],
    call_data: true,
    logs: true,
    |
    info: CallInfo,
    call_data: swapCall,
    log_data: UniswapV2SwapCallLogs,
    db_tx: &DB| {
        let logs = log_data.swap_field?;
        let recipient = call_data.to;

        let details = db_tx.get_protocol_details_sorted(info.target_address)?;
        let [token_0, token_1] = [details.token0, details.token1];

        let t0_info = db_tx.try_fetch_token_info(token_0)?;
        let t1_info = db_tx.try_fetch_token_info(token_1)?;

        println!("UniswapV2 logs: {:?}", logs);
        println!("UniswapV2 call_data: {:?}", call_data);
        println!("UniswapV2 tokens: 0:{:?}, 1:{:?}", t0_info, t1_info);
        if logs.amount0In == U256::ZERO {
            let amount_in = logs.amount1In.to_scaled_rational(t1_info.decimals);
            let amount_out = logs.amount0Out.to_scaled_rational(t0_info.decimals);

            return Ok(NormalizedSwap {
            protocol: Protocol::UniswapV2,
                pool: info.target_address,
                trace_index: info.trace_idx,
                from: info.from_address,
                recipient,
                token_in: t1_info,
                token_out: t0_info,
                amount_in,
                amount_out,
                msg_value: info.msg_value
            })
        } else {
            /*
            if this else statement is reached, that means:
                amount0In != 0
                amount1In = 0
                amount0Out = 0
                amount1Out != 0
             */
            let amount_in = logs.amount0In.to_scaled_rational(t0_info.decimals);

            // since amount1Out != 0 must be true as per above, the else statement here will never be reached
            let amount_out = if logs.amount1Out == U256::ZERO {
                // this should use t0_info instead of t1 since it's token0
                logs.amount1Out.to_scaled_rational(t1_info.decimals)
               // logs.amount0Out.to_scaled_rational(t1_info.decimals)
            } else {
                logs.amount1Out.to_scaled_rational(t1_info.decimals)
            };

            return Ok(NormalizedSwap {
                protocol: Protocol::UniswapV2,
                pool: info.target_address,
                trace_index: info.trace_idx,
                from: info.from_address,
                recipient,
                token_in: t0_info,
                token_out: t1_info,
                amount_in,
                amount_out,
                msg_value: info.msg_value
            })
        }
    }
);

action_impl!(
    Protocol::UniswapV2,
    crate::UniswapV2::mintCall,
    Mint,
    [..Mint],
    logs: true,
    call_data: true,
    |
     info: CallInfo,
     call_data: mintCall,
     log_data: UniswapV2MintCallLogs,
     db_tx: &DB| {
        let log_data = log_data.mint_field?;
        let details = db_tx.get_protocol_details_sorted(info.target_address)?;
        let [token_0, token_1] = [details.token0, details.token1];

        let t0_info = db_tx.try_fetch_token_info(token_0)?;
        let t1_info = db_tx.try_fetch_token_info(token_1)?;

        let am0 = log_data.amount0.to_scaled_rational(t0_info.decimals);
        let am1 = log_data.amount1.to_scaled_rational(t1_info.decimals);

        Ok(NormalizedMint {
            protocol: Protocol::UniswapV2,
            recipient: call_data.to,
            from: info.from_address,
            trace_index: info.trace_idx,
            pool: info.target_address,
            token: vec![t0_info, t1_info],
            amount: vec![am0, am1],
        })
    }
);
action_impl!(
    Protocol::UniswapV2,
    crate::UniswapV2::burnCall,
    Burn,
    [..Burn],
    call_data: true,
    logs: true,
    |
    info: CallInfo,
    call_data: burnCall,
     log_data: UniswapV2BurnCallLogs,
     db_tx: &DB| {
        let log_data = log_data.burn_field?;
        let details = db_tx.get_protocol_details_sorted(info.target_address)?;
        let [token_0, token_1] = [details.token0, details.token1];

        let t0_info = db_tx.try_fetch_token_info(token_0)?;
        let t1_info = db_tx.try_fetch_token_info(token_1)?;

        let am0 = log_data.amount0.to_scaled_rational(t0_info.decimals);
        let am1 = log_data.amount1.to_scaled_rational(t1_info.decimals);

        Ok(NormalizedBurn {
            protocol: Protocol::UniswapV2,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: call_data.to,
            pool: info.target_address,
            token: vec![t0_info, t1_info],
            amount: vec![am0, am1],
        })
    }
);
