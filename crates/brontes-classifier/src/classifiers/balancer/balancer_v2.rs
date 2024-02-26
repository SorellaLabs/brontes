use alloy_primitives::{Address, FixedBytes};
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::NormalizedSwap, structured_trace::CallInfo, ToScaledRational,
};
use reth_primitives::U256;

action_impl!(
    Protocol::BalancerV2,
    crate::BalancerV2Vault::swapCall, 
    Swap,
    [..Swap],
    call_data: true,
    logs: true,
    |info: CallInfo, call_data: swapCall, log_data: BalancerV2swapCallLogs, db: &DB| { 

        let token_in = db.try_fetch_token_info(log_data.Swap_field.tokenIn)?;
        let token_out = db.try_fetch_token_info(log_data.Swap_field.tokenOut)?;
        let amount_in = log_data.Swap_field.amountIn.to_scaled_rational(token_in.decimals);
        let amount_out = log_data.Swap_field.amountOut.to_scaled_rational(token_out.decimals);

        let pool_address = pool_id_to_address(log_data.Swap_field.poolId);

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

// ~ https://docs.balancer.fi/reference/contracts/pool-interfacing.html#poolids
// The poolId is a unique identifier, the first portion of which is the pool's contract address. 
// For example, the pool with the id 0x5c6ee304399dbdb9c8ef030ab642b10820db8f56000200000000000000000014 has a contract address of 0x5c6ee304399dbdb9c8ef030ab642b10820db8f56.
fn pool_id_to_address(pool_id: FixedBytes<32>) -> Address {
    Address::from_slice(&pool_id[0..20])
}