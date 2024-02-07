use alloy_primitives::{Address, U256};
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::{NormalizedBurn, NormalizedCollect, NormalizedMint, NormalizedSwap},
    structured_trace::CallInfo,
    ToScaledRational,
};

use crate::UniswapV3::{burnReturn, collectReturn, mintReturn, swapReturn};

action_impl!(
    Protocol::UniswapV3,
    crate::UniswapV3::swapCall,
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
        let tokens = db_tx.get_protocol_tokens(info.target_address).ok()??;
        let [token_0, token_1] = [tokens.token0, tokens.token1];

        let t0_info = db_tx.try_fetch_token_info(token_0).ok()??;
        let t1_info = db_tx.try_fetch_token_info(token_1).ok()??;

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

        Some(NormalizedSwap {
            protocol: Protocol::UniswapV3,
            trace_index: info.trace_idx,
            from: info.from_address,
            pool: info.target_address,
            recipient,
            token_in,
            token_out,
            amount_in,
            amount_out,
        })
    }
);
action_impl!(
    Protocol::UniswapV3,
    crate::UniswapV3::mintCall,
    Mint,
    [Mint],
    return_data: true,
    call_data: true,
     |
     info: CallInfo,
     call_data: mintCall,
     return_data: mintReturn,  db_tx: &DB| {
        let token_0_delta = return_data.amount0;
        let token_1_delta = return_data.amount1;
        let tokens = db_tx.get_protocol_tokens(info.target_address).ok()??;
        let [token_0, token_1] = [tokens.token0, tokens.token1];

        let t0_info = db_tx.try_fetch_token_info(token_0).ok()??;
        let t1_info = db_tx.try_fetch_token_info(token_1).ok()??;

        let am0 = token_0_delta.to_scaled_rational(t0_info.decimals);
        let am1 = token_1_delta.to_scaled_rational(t1_info.decimals);

        Some(NormalizedMint {
            protocol: Protocol::UniswapV3,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: call_data.recipient,
            to: info.target_address,
            token: vec![t0_info, t1_info],
            amount: vec![am0, am1],
        })
    }
);
action_impl!(
    Protocol::UniswapV3,
    crate::UniswapV3::burnCall,
    Burn,
    [Burn],
    return_data: true,
    |
    info: CallInfo,
    return_data: burnReturn,
    db_tx: &DB| {
        let token_0_delta: U256 = return_data.amount0;
        let token_1_delta: U256 = return_data.amount1;
        let tokens = db_tx.get_protocol_tokens(info.target_address).ok()??;
        let [token_0, token_1] = [tokens.token0, tokens.token1];

        let t0_info = db_tx.try_fetch_token_info(token_0).ok()??;
        let t1_info = db_tx.try_fetch_token_info(token_1).ok()??;

        let am0 = token_0_delta.to_scaled_rational(t0_info.decimals);
        let am1 = token_1_delta.to_scaled_rational(t1_info.decimals);

        Some(NormalizedBurn {
            protocol: Protocol::UniswapV3,
            to: target_address,
            recipient: target_address,
            trace_index,
            from: from_address,
            token: vec![t0_info, t1_info],
            amount: vec![am0, am1],
        })
    }
);
action_impl!(
    Protocol::UniswapV3,
    crate::UniswapV3::collectCall,
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
        let tokens = db_tx.get_protocol_tokens(info.target_address).ok()??;
        let [token_0, token_1] = [tokens.token0, tokens.token1];

        let t0_info = db_tx.try_fetch_token_info(token_0).ok()??;
        let t1_info = db_tx.try_fetch_token_info(token_1).ok()??;

        let am0 = return_data.amount0.to_scaled_rational(t0_info.decimals);
        let am1 = return_data.amount1.to_scaled_rational(t1_info.decimals);

        Some(NormalizedCollect {
            protocol: Protocol::UniswapV3,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: call_data.recipient,
            to: info.to_address,
            token: vec![t0_info, t1_info],
            amount: vec![am0, am1],
        })
    }
);
