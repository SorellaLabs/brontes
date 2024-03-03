use alloy_primitives::{Address, FixedBytes};
use brontes_core::{DBWriter, LibmdbxReader};
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    db::token_info::TokenInfoWithAddress,
    normalized_actions::{NormalizedBatch, NormalizedBurn, NormalizedMint, NormalizedSwap},
    structured_trace::CallInfo,
    ToScaledRational,
};
use eyre::Error;
use malachite::Rational;
use reth_primitives::U256;

use crate::BalancerV2Vault::PoolBalanceChanged;

action_impl!(
    Protocol::BalancerV2,
    crate::BalancerV2Vault::swapCall,
    Swap,
    [..Swap],
    call_data: true,
    logs: true,
    |info: CallInfo, call_data: swapCall, log_data: BalancerV2SwapCallLogs, db: &DB| {
        let swap_field = log_data.swap_field?;

        let token_in = db.try_fetch_token_info(swap_field.tokenIn)?;
        let token_out = db.try_fetch_token_info(swap_field.tokenOut)?;
        let amount_in = swap_field.amountIn.to_scaled_rational(token_in.decimals);
        let amount_out = swap_field.amountOut.to_scaled_rational(token_out.decimals);

        let pool_address = pool_id_to_address(swap_field.poolId);

        Ok(NormalizedSwap {
            protocol: Protocol::BalancerV2,
            trace_index: info.trace_idx,
            from: call_data.funds.sender,
            recipient: call_data.funds.recipient,
            pool: pool_address,
            token_in,
            token_out,
            amount_in,
            amount_out,
            msg_value: U256::ZERO,
        })
    }
);

action_impl!(
    Protocol::BalancerV2,
    crate::BalancerV2Vault::batchSwapCall,
    Batch,
    [..Swap*],
    call_data: true,
    logs: true,
    |info: CallInfo, call_data: batchSwapCall, log_data: BalancerV2BatchSwapCallLogs, db: &DB| {
        let swap_field = log_data.swap_field?;

        let swaps_count = swap_field.len();
        let mut normalized_swaps = Vec::new();

        for (index, swap_log) in swap_field.iter().enumerate() {
            let token_in = db.try_fetch_token_info(swap_log.tokenIn)?;
            let token_out = db.try_fetch_token_info(swap_log.tokenOut)?;
            let amount_in = swap_log.amountIn.to_scaled_rational(token_in.decimals);
            let amount_out = swap_log.amountOut.to_scaled_rational(token_out.decimals);
            let pool_address = pool_id_to_address(swap_log.poolId);

            let (from, recipient) = match index {
                0 if swaps_count > 1 => (call_data.funds.sender, info.target_address),
                0 => (call_data.funds.sender, call_data.funds.recipient),
                _ if index == swaps_count - 1 => (info.target_address, call_data.funds.recipient),
                _ => (info.target_address, info.target_address),
            };

            normalized_swaps.push(NormalizedSwap {
                protocol: Protocol::BalancerV2,
                trace_index: info.trace_idx,
                from,
                recipient,
                pool: pool_address,
                token_in,
                token_out,
                amount_in,
                amount_out,
                msg_value: U256::ZERO,
            });
        }

        Ok(NormalizedBatch {
            protocol: Protocol::BalancerV2,
            trace_index: info.trace_idx,
            solver: info.msg_sender,
            settlement_contract: info.target_address,
            user_swaps: normalized_swaps,
            solver_swaps: None,
            msg_value: info.msg_value
        })
    }
);

fn process_pool_balance_changes<DB: LibmdbxReader + DBWriter>(
    logs: &PoolBalanceChanged,
    db: &DB,
) -> Result<(Vec<TokenInfoWithAddress>, Vec<Rational>), Error> {
    let mut tokens = Vec::new();
    let mut amounts = Vec::new();

    for (i, &token_address) in logs.tokens.iter().enumerate() {
        let token = db.try_fetch_token_info(token_address)?;
        let amount = logs.deltas[i].abs().to_scaled_rational(token.decimals);
        tokens.push(token);
        amounts.push(amount);
    }

    Ok((tokens, amounts))
}

action_impl!(
    Protocol::BalancerV2,
    crate::BalancerV2Vault::joinPoolCall,
    Mint,
    [..PoolBalanceChanged],
    call_data: true,
    logs: true,
    |info: CallInfo, call_data: joinPoolCall, log_data: BalancerV2JoinPoolCallLogs, db: &DB| {
        let logs = log_data.pool_balance_changed_field?;
        let (tokens, amounts) = process_pool_balance_changes(&logs, db)?;

        Ok(NormalizedMint {
            protocol: Protocol::BalancerV2,
            trace_index: info.trace_idx,
            from: call_data.sender,
            recipient: call_data.recipient,
            pool: pool_id_to_address(call_data.poolId),
            token: tokens,
            amount: amounts
        })
    }
);

action_impl!(
    Protocol::BalancerV2,
    crate::BalancerV2Vault::exitPoolCall,
    Burn,
    [..PoolBalanceChanged],
    call_data: true,
    logs: true,
    |info: CallInfo, call_data: exitPoolCall, log_data: BalancerV2ExitPoolCallLogs, db: &DB| {
        let logs = log_data.pool_balance_changed_field?;
        let (tokens, amounts) = process_pool_balance_changes(&logs, db)?;

        Ok(NormalizedBurn {
            protocol: Protocol::BalancerV2,
            trace_index: info.trace_idx,
            from: call_data.sender,
            recipient: call_data.recipient,
            pool: pool_id_to_address(call_data.poolId),
            token: tokens,
            amount: amounts
        })
    }
);

// ~ https://docs.balancer.fi/reference/contracts/pool-interfacing.html#poolids
// The poolId is a unique identifier, the first portion of which is the pool's
// contract address. For example, the pool with the id
// 0x5c6ee304399dbdb9c8ef030ab642b10820db8f56000200000000000000000014 has a
// contract address of 0x5c6ee304399dbdb9c8ef030ab642b10820db8f56.
fn pool_id_to_address(pool_id: FixedBytes<32>) -> Address {
    Address::from_slice(&pool_id[0..20])
}
