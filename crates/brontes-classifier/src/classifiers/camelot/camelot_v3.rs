use alloy_primitives::U256;
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::{NormalizedBurn, NormalizedCollect, NormalizedMint, NormalizedSwap},
    structured_trace::CallInfo,
    ToScaledRational,
};

action_impl!(
    Protocol::CamelotV3,
    crate::CamelotV3::swapCall,
    Swap,
    [Swap],
    call_data: true,
    return_data: true,
    |
    info: CallInfo,
    call_data: swapCall,
    return_data: swapReturn,
    db_tx: &DB| {
        let token_0_delta = return_data.amount0;
        let token_1_delta = return_data.amount1;
        let recipient = call_data.recipient;
        let details = db_tx.get_protocol_details_sorted(info.target_address)?;
        let [token_0, token_1] = [details.token0, details.token1];

        let t0_info = db_tx.try_fetch_token_info(token_0)?;
        let t1_info = db_tx.try_fetch_token_info(token_1)?;

        let (amount_in, amount_out, token_in, token_out) = if token_0_delta.is_negative() {
            (
                token_1_delta.to_scaled_rational(t1_info.decimals),
                token_0_delta.abs().to_scaled_rational(t0_info.decimals),
                t1_info,
                t0_info,
            )
        } else {
            (
                token_0_delta.to_scaled_rational(t0_info.decimals),
                token_1_delta.abs().to_scaled_rational(t1_info.decimals),
                t0_info,
                t1_info,
            )
        };

        Ok(NormalizedSwap {
            protocol: Protocol::CamelotV3,
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
    Protocol::CamelotV3,
    crate::CamelotV3::mintCall,
    Mint,
    [Mint],
    return_data: true,
    call_data: true,
    logs: true,
    |
        info: CallInfo,
    call_data: mintCall,
     return_data: mintReturn, logs: CamelotV3MintCallLogs,  db_tx: &DB| {

        let owner = logs.mint_field?.owner;
        let token_0_delta = return_data.amount0;
        let token_1_delta = return_data.amount1;
        let details = db_tx.get_protocol_details_sorted(info.target_address)?;
        let [token_0, token_1] = [details.token0, details.token1];

        let t0_info = db_tx.try_fetch_token_info(token_0)?;
        let t1_info = db_tx.try_fetch_token_info(token_1)?;

        let am0 = token_0_delta.to_scaled_rational(t0_info.decimals);
        let am1 = token_1_delta.to_scaled_rational(t1_info.decimals);


        Ok(NormalizedMint {
            protocol:Protocol::CamelotV3,
            trace_index: info.trace_idx,
            from: owner,
            recipient: call_data.recipient,
            pool: info.target_address,
            token: vec![t0_info, t1_info],
            amount: vec![am0, am1],
        })
    }
);

action_impl!(
    Protocol::CamelotV3,
    crate::CamelotV3::burnCall,
    Burn,
    [Burn],
    return_data: true,
    |
    info: CallInfo,
    return_data: burnReturn,
    db_tx: &DB| {
        let token_0_delta: U256 = return_data.amount0;
        let token_1_delta: U256 = return_data.amount1;
        let details = db_tx.get_protocol_details_sorted(info.target_address)?;
        let [token_0, token_1] = [details.token0, details.token1];

        let t0_info = db_tx.try_fetch_token_info(token_0)?;
        let t1_info = db_tx.try_fetch_token_info(token_1)?;

        let am0 = token_0_delta.to_scaled_rational(t0_info.decimals);
        let am1 = token_1_delta.to_scaled_rational(t1_info.decimals);

        Ok(NormalizedBurn {
            protocol: Protocol::CamelotV3,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: info.target_address,
            pool: info.target_address,
            token: vec![t0_info, t1_info],
            amount: vec![am0, am1],
        })
    }
);
action_impl!(
    Protocol::CamelotV3,
    crate::CamelotV3::collectCall,
    Collect,
    [Collect],
    call_data: true,
    return_data: true,
    |
    info: CallInfo,
    call_data: collectCall,
    return_data: collectReturn,
    db_tx: &DB
    | {
        let details = db_tx.get_protocol_details_sorted(info.target_address)?;
        let [token_0, token_1] = [details.token0, details.token1];

        let t0_info = db_tx.try_fetch_token_info(token_0)?;
        let t1_info = db_tx.try_fetch_token_info(token_1)?;

        let am0 = return_data.amount0.to_scaled_rational(t0_info.decimals);
        let am1 = return_data.amount1.to_scaled_rational(t1_info.decimals);
        Ok(NormalizedCollect {
            protocol: Protocol::CamelotV3,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: call_data.recipient,
            pool: info.target_address,
            token: vec![t0_info, t1_info],
            amount: vec![am0, am1],
        })
    }
);
