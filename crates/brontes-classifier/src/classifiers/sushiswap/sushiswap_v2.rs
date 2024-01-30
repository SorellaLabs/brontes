use alloy_primitives::{Address, U256};
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::{NormalizedBurn, NormalizedMint, NormalizedSwap},
    ToScaledRational,
};

action_impl!(
    Protocol::SushiSwapV2,
    crate::SushiSwapV2::swapCall,
    Swap,
    [..Swap],
    call_data: true,
    logs: true,
    |trace_index,
    from_address: Address,
    target_address: Address,
    _msg_sender: Address,
    call_data: swapCall,
    logs: SushiSwapV2swapCallLogs,
    db_tx: &DB| {
        let logs = logs.Swap_field;

        let recipient = call_data.to;
        let tokens = db_tx.get_protocol_tokens(target_address).ok()??;
        let [token_0, token_1] = [tokens.token0, tokens.token1];

        let t0_info = db_tx.try_get_token_info(token_0).ok()??;
        let t1_info = db_tx.try_get_token_info(token_1).ok()??;

        if logs.amount0In == U256::ZERO {
            let amount_in = logs.amount1In.to_scaled_rational(t1_info.decimals);
            let amount_out = logs.amount0Out.to_scaled_rational(t0_info.decimals);

            return Some(NormalizedSwap {
                protocol: Protocol::SushiSwapV2,
                pool: target_address,
                trace_index,
                from: from_address,
                recipient,
                token_in: t1_info,
                token_out: t0_info,
                amount_in,
                amount_out,
            })
        } else {
            let amount_in = logs.amount0In.to_scaled_rational(t0_info.decimals);
            let amount_out = logs.amount1Out.to_scaled_rational(t1_info.decimals);
            return Some(NormalizedSwap {
                protocol: Protocol::SushiSwapV2,
                trace_index,
                pool: target_address,
                from: from_address,
                recipient,
                token_in: t0_info,
                token_out: t1_info,
                amount_in,
                amount_out,
            })
        }
    }
);

action_impl!(
    Protocol::SushiSwapV2,
    crate::SushiSwapV2::mintCall,
    Mint,
    // can be a double transfer if the pool has no liquidity
    [..Mint],
    logs: true,
    call_data: true,
    |trace_index,
     from_address: Address,
     target_address: Address,
     _msg_sender: Address,
     call_data: mintCall,
     log_data: SushiSwapV2mintCallLogs,
     db_tx: &DB| {
        let log_data = log_data.Mint_field;

        let tokens = db_tx.get_protocol_tokens(target_address).ok()??;
        let [token_0, token_1] = [tokens.token0, tokens.token1];

        let t0_info = db_tx.try_get_token_info(token_0).ok()??;
        let t1_info = db_tx.try_get_token_info(token_1).ok()??;

        let am0 = log_data.amount0.to_scaled_rational(t0_info.decimals);
        let am1 = log_data.amount1.to_scaled_rational(t1_info.decimals);

        Some(NormalizedMint {
            protocol: Protocol::SushiSwapV2,
            recipient: call_data.to,
            from: from_address,
            trace_index,
            to: target_address,
            token: vec![t0_info, t1_info],
            amount: vec![am0, am1],
        })
    }
);

action_impl!(
    Protocol::SushiSwapV2,
    crate::SushiSwapV2::burnCall,
    Burn,
    [..Burn],
    call_data: true,
    logs: true,
    |trace_index,
     from_address: Address,
     target_address: Address,
     _msg_sender: Address,
     call_data: burnCall,
     log_data: SushiSwapV2burnCallLogs,
     db_tx: &DB| {
        let log_data = log_data.Burn_field;
        let tokens = db_tx.get_protocol_tokens(target_address).ok()??;
        let [token_0, token_1] = [tokens.token0, tokens.token1];

        let t0_info = db_tx.try_get_token_info(token_0).ok()??;
        let t1_info = db_tx.try_get_token_info(token_1).ok()??;

        let am0 = log_data.amount0.to_scaled_rational(t0_info.decimals);
        let am1 = log_data.amount1.to_scaled_rational(t1_info.decimals);

        Some(NormalizedBurn {
            protocol: Protocol::SushiSwapV2,
            recipient: call_data.to,
            to: target_address,
            trace_index,
            from: from_address,
            token: vec![t0_info, t1_info],
            amount: vec![am0, am1],
        })
    }
);
