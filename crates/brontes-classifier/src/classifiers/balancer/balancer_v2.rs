use alloy_primitives::{Address, FixedBytes};
use brontes_database::libmdbx::{DBWriter, LibmdbxReader};
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    db::token_info::TokenInfoWithAddress,
    normalized_actions::{Actions, NormalizedAggregator, NormalizedBatch, NormalizedBurn, NormalizedMint, NormalizedSwap},
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
            msg_value: info.msg_value,
        })
    }
);

action_impl!(
    Protocol::BalancerV2,
    crate::BalancerV2Vault::batchSwapCall,
    Aggregator,
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

        let child_actions: Vec<Actions> = normalized_swaps.into_iter().map(|d| Actions::Swap(d)).collect();

        Ok(NormalizedAggregator {
            protocol: Protocol::BalancerV2,
            trace_index: info.trace_idx,
            from: call_data.funds.sender,
            recipient: call_data.funds.sender,
            child_actions,
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
        if logs.deltas[i].is_zero() {
            continue;
        }

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


#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{hex, B256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        db::token_info::TokenInfo,
        Protocol::BalancerV2,
        TreeSearchBuilder,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_balancer_v2_swap() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let swap =
            B256::from(hex!("da10a5e3cb8c34c77634cb9a1cfe02ec2b23029f1f288d79b6252b2f8cae20d3"));

        let eq_action = Actions::Swap(NormalizedSwap {
            protocol:    BalancerV2,
            trace_index: 0,
            from:        Address::new(hex!("5d2146eAB0C6360B864124A99BD58808a3014b5d")),
            recipient:   Address::new(hex!("5d2146eAB0C6360B864124A99BD58808a3014b5d")),
            pool:        Address::new(hex!("358e056c50eea4ca707e891404e81d9b898d0b41")),
            token_in:    TokenInfoWithAddress::weth(),
            amount_in:   U256::from_str("10000000000000000").unwrap().to_scaled_rational(18),
            token_out:   TokenInfoWithAddress {
                address: Address::new(hex!("6C22910c6F75F828B305e57c6a54855D8adeAbf8")),
                inner:   TokenInfo { decimals: 9, symbol: "SATS".to_string() },
            },
            amount_out:  U256::from_str("7727102831493")
                .unwrap()
                .to_scaled_rational(9),

            msg_value: U256::from_str("10000000000000000").unwrap(),
        });

        classifier_utils
            .contains_action(
                swap,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Actions::is_swap),
            )
            .await
            .unwrap();
    }

    #[brontes_macros::test]
    async fn test_balancer_v2_join_pool() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let mint =
            B256::from(hex!("ffed34d6f2d9e239b5cd3985840a37f1fa0c558edcd1a2f3d2b8bd7f314ef6a3"));

        let eq_action = Actions::Mint(NormalizedMint{ 
            protocol: Protocol::BalancerV2, 
            trace_index: 0, 
            from: Address::new(hex!("750c31d2290c456fcca1c659b6add80e7a88f881")), 
            recipient: Address::new(hex!("750c31d2290c456fcca1c659b6add80e7a88f881")), 
            pool: Address::new(hex!("848a5564158d84b8A8fb68ab5D004Fae11619A54")), 
            token: vec![
                TokenInfoWithAddress {
                    address: Address::new(hex!("cd5fe23c85820f7b72d0926fc9b05b43e359b7ee")),
                    inner: TokenInfo { decimals: 18, symbol: "weETH".to_string() }
                }
            ],
            amount: vec![
                U256::from_str("1935117712922949743").unwrap().to_scaled_rational(18),
            ] 
        });

        classifier_utils
            .contains_action(
                mint,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Actions::is_mint),
            )
            .await
            .unwrap();
    }

    #[brontes_macros::test]
    async fn test_balancer_v2_exit_pool() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let burn =
            B256::from(hex!("ad13973ee8e507b36adc5d28dc53b77d58d00d5ac6a09aa677936be8aaf6c8a1"));

        let eq_action = Actions::Burn(NormalizedBurn{ 
            protocol: Protocol::BalancerV2, 
            trace_index: 0, 
            from: Address::new(hex!("f4283d13ba1e17b33bb3310c3149136a2ef79ef7")), 
            recipient: Address::new(hex!("f4283d13ba1e17b33bb3310c3149136a2ef79ef7")), 
            pool: Address::new(hex!("848a5564158d84b8A8fb68ab5D004Fae11619A54")), 
            token: vec![
                TokenInfoWithAddress {
                    address: Address::new(hex!("bf5495efe5db9ce00f80364c8b423567e58d2110")),
                    inner: TokenInfo { decimals: 18, symbol: "ezETH".to_string() }
                },
                TokenInfoWithAddress {
                    address: Address::new(hex!("cd5fe23c85820f7b72d0926fc9b05b43e359b7ee")),
                    inner: TokenInfo { decimals: 18, symbol: "weETH".to_string() }
                },
                TokenInfoWithAddress {
                    address: Address::new(hex!("fae103dc9cf190ed75350761e95403b7b8afa6c0")),
                    inner: TokenInfo { decimals: 18, symbol: "rswETH".to_string() }
                }
            ],
            amount: vec![
                U256::from_str("471937215318872937").unwrap().to_scaled_rational(18),
                U256::from_str("757823171697267931").unwrap().to_scaled_rational(18),
                U256::from_str("699970729674926490").unwrap().to_scaled_rational(18)
            ] 
        });

        classifier_utils
            .contains_action(
                burn,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Actions::is_burn),
            )
            .await
            .unwrap();
    }
}