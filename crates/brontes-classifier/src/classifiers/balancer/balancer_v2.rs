use alloy_primitives::{Address, FixedBytes};
use brontes_database::libmdbx::{DBWriter, LibmdbxReader};
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    db::token_info::TokenInfoWithAddress,
    normalized_actions::{
        NormalizedBurn, NormalizedFlashLoan, NormalizedMint, NormalizedNewPool,
        NormalizedPoolConfigUpdate,
    },
    structured_trace::CallInfo,
    ToScaledRational,
};
use eyre::Error;
use malachite::Rational;

use crate::BalancerV2Vault::PoolBalanceChanged;

/*
action_impl!(
    Protocol::BalancerV2,
    crate::IGeneralPool::onSwapCall,
    Swap,
    [..],
    call_data: true,
    return_data: true,
    |info: CallInfo, call_data: onSwapCall, return_data: onSwapReturn, db: &DB| {
        let pool = pool_id_to_address(call_data.swapRequest.poolId);
        let token_in = db.try_fetch_token_info(call_data.swapRequest.tokenIn)?;
        let token_out = db.try_fetch_token_info(call_data.swapRequest.tokenOut)?;
        let amount_in = call_data.swapRequest.amount.to_scaled_rational(token_in.decimals);
        let amount_out = return_data.amount.to_scaled_rational(token_out.decimals);

        Ok(NormalizedSwap {
            protocol: Protocol::BalancerV2,
            trace_index: info.trace_idx,
            from: call_data.swapRequest.from,
            recipient: call_data.swapRequest.to,
            pool,
            token_in,
            amount_in,
            token_out,
            amount_out,
            msg_value: U256::ZERO,
        })
    }
);


action_impl!(
    Protocol::BalancerV2,
    crate::IGeneralPool::onSwap_0Call,
    Swap,
    [..],
    call_data: true,
    return_data: true,
    |info: CallInfo, call_data: onSwap_0Call, return_data: onSwap_0Return, db: &DB| {
        let pool = pool_id_to_address(call_data.swapRequest.poolId);
        let token_in = db.try_fetch_token_info(call_data.swapRequest.tokenIn)?;
        let token_out = db.try_fetch_token_info(call_data.swapRequest.tokenOut)?;
        let amount_in = call_data.swapRequest.amount.to_scaled_rational(token_in.decimals);
        let amount_out = return_data.amount.to_scaled_rational(token_out.decimals);

        Ok(NormalizedSwap {
            protocol: Protocol::BalancerV2,
            trace_index: info.trace_idx,
            from: call_data.swapRequest.from,
            recipient: call_data.swapRequest.to,
            pool,
            token_in,
            amount_in,
            token_out,
            amount_out,
            msg_value: U256::ZERO,
        })
    }
);

action_impl!(
    Protocol::BalancerV2,
    crate::IMinimalSwapInfoPool::onSwap_1Call,
    Swap,
    [..],
    call_data: true,
    return_data: true,
    |info: CallInfo, call_data: onSwap_1Call, return_data: onSwap_1Return, db: &DB| {
        let pool = pool_id_to_address(call_data.swapRequest.poolId);
        let token_in = db.try_fetch_token_info(call_data.swapRequest.tokenIn)?;
        let token_out = db.try_fetch_token_info(call_data.swapRequest.tokenOut)?;
        let amount_in = call_data.swapRequest.amount.to_scaled_rational(token_in.decimals);
        let amount_out = return_data.amount.to_scaled_rational(token_out.decimals);

        Ok(NormalizedSwap {
            protocol: Protocol::BalancerV2,
            trace_index: info.trace_idx,
            from: call_data.swapRequest.from,
            recipient: call_data.swapRequest.to,
            pool,
            token_in,
            amount_in,
            token_out,
            amount_out,
            msg_value: U256::ZERO,
        })
    }
);

*/

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
    crate::BalancerV2Vault::flashLoanCall,
    FlashLoan,
    [..FlashLoan*],
    call_data: true,
    |info: CallInfo, call_data: flashLoanCall, db: &DB| {
        let (assets, amounts): (Vec<TokenInfoWithAddress>, Vec<Rational>) = call_data.tokens
            .iter()
            .zip(call_data.amounts.iter())
            .map(|(token_address, amount)| {
                let token = db.try_fetch_token_info(*token_address)?;
                let amount = amount.to_scaled_rational(token.decimals);
                Ok((token, amount))
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e: Error| <Error as Into<eyre::ErrReport>>::into(e))
            .map(|v| v.into_iter().unzip())?;

        Ok(NormalizedFlashLoan {
            protocol: Protocol::BalancerV2,
            trace_index: info.trace_idx,
            from: info.from_address,
            pool: info.target_address,
            receiver_contract: call_data.recipient,
            assets,
            amounts,
            aave_mode: None,
            child_actions: vec![],
            repayments: vec![],
            fees_paid: vec![],
            msg_value: info.msg_value
        })
    }
);

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

action_impl!(
    Protocol::BalancerV2,
    crate::BalancerV2Vault::registerPoolCall,
    NewPool,
    [..PoolRegistered],
    logs: true,
    |info: CallInfo, log_data: BalancerV2RegisterPoolCallLogs, _| {
        let logs = log_data.pool_registered_field?;

        Ok(NormalizedNewPool {
            trace_index: info.trace_idx,
            protocol: Protocol::BalancerV2,
            pool_address: logs.poolAddress,
            tokens: vec![],
        })
    }
);

action_impl!(
    Protocol::BalancerV2,
    crate::BalancerV2Vault::registerTokensCall,
    PoolConfigUpdate,
    [..TokensRegistered],
    logs: true,
    |info: CallInfo, log_data: BalancerV2RegisterTokensCallLogs, _| {
        let logs = log_data.tokens_registered_field?;
        let pool_address = pool_id_to_address(logs.poolId);

        Ok(NormalizedPoolConfigUpdate{
            trace_index: info.trace_idx,
            protocol: Protocol::BalancerV2,
            pool_address,
            tokens: logs.tokens
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
    use brontes_types::{db::token_info::TokenInfo, normalized_actions::Action, TreeSearchBuilder};

    use super::*;

    /*
    #[brontes_macros::test]
    async fn test_balancer_v2_swap() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let swap =
            B256::from(hex!("da10a5e3cb8c34c77634cb9a1cfe02ec2b23029f1f288d79b6252b2f8cae20d3"));

        classifier_utils.ensure_token(TokenInfoWithAddress {
            address: Address::new(hex!("6C22910c6F75F828B305e57c6a54855D8adeAbf8")),
            inner: TokenInfo { decimals: 9, symbol: "SATS".to_string() },
        });

        classifier_utils.ensure_protocol(
            Protocol::BalancerV2,
            hex!("358e056c50eea4ca707e891404e81d9b898d0b41").into(),
            WETH_ADDRESS,
            Some(hex!("6C22910c6F75F828B305e57c6a54855D8adeAbf8").into()),
            None,
            None,
            None,
            None,
        );

        // Minimal swap
        let eq_action = Action::Swap(NormalizedSwap {
            protocol: BalancerV2,
            trace_index: 1,
            from: Address::new(hex!("5d2146eAB0C6360B864124A99BD58808a3014b5d")),
            recipient: Address::new(hex!("5d2146eAB0C6360B864124A99BD58808a3014b5d")),
            pool: Address::new(hex!("358e056c50eea4ca707e891404e81d9b898d0b41")),
            token_in: TokenInfoWithAddress::weth(),
            amount_in: U256::from_str("10000000000000000")
                .unwrap()
                .to_scaled_rational(18),
            token_out: TokenInfoWithAddress {
                address: Address::new(hex!("6C22910c6F75F828B305e57c6a54855D8adeAbf8")),
                inner: TokenInfo { decimals: 9, symbol: "SATS".to_string() },
            },
            amount_out: U256::from_str("7727102831493")
                .unwrap()
                .to_scaled_rational(9),

            msg_value: U256::ZERO,
        });

        classifier_utils
            .contains_action(
                swap,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Action::is_swap),
            )
            .await
            .unwrap();
    }
    */

    #[brontes_macros::test]
    async fn test_balancer_v2_flash_loan() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let flash_loan =
            B256::from(hex!("0feed8bde2117cc166264dfeebfdec0cf6dc6655325fb94bd90f00688f8c463a"));

        classifier_utils.ensure_token(TokenInfoWithAddress {
            address: Address::new(hex!("cd5fe23c85820f7b72d0926fc9b05b43e359b7ee")),
            inner: TokenInfo { decimals: 18, symbol: "weETH".to_string() },
        });

        let eq_action = Action::FlashLoan(NormalizedFlashLoan {
            protocol: Protocol::BalancerV2,
            trace_index: 3,
            from: Address::new(hex!("97c1a26482099363cb055f0f3ca1d6057fe55447")),
            pool: Address::new(hex!("ba12222222228d8ba445958a75a0704d566bf2c8")),
            receiver_contract: Address::new(hex!("97c1a26482099363cb055f0f3ca1d6057fe55447")),
            assets: vec![TokenInfoWithAddress {
                address: Address::new(hex!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")),
                inner: TokenInfo { decimals: 18, symbol: "WETH".to_string() },
            }],
            amounts: vec![U256::from_str("653220647374307183")
                .unwrap()
                .to_scaled_rational(18)],
            aave_mode: None,
            child_actions: vec![],
            repayments: vec![],
            fees_paid: vec![],
            msg_value: U256::ZERO,
        });

        classifier_utils
            .contains_action_except(
                flash_loan,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Action::is_flash_loan),
                &["child_actions", "repayments"],
            )
            .await
            .unwrap();
    }

    #[brontes_macros::test]
    async fn test_balancer_v2_join_pool() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let mint =
            B256::from(hex!("ffed34d6f2d9e239b5cd3985840a37f1fa0c558edcd1a2f3d2b8bd7f314ef6a3"));

        classifier_utils.ensure_token(TokenInfoWithAddress {
            address: Address::new(hex!("cd5fe23c85820f7b72d0926fc9b05b43e359b7ee")),
            inner: TokenInfo { decimals: 18, symbol: "weETH".to_string() },
        });

        let eq_action = Action::Mint(NormalizedMint {
            protocol: Protocol::BalancerV2,
            trace_index: 0,
            from: Address::new(hex!("750c31d2290c456fcca1c659b6add80e7a88f881")),
            recipient: Address::new(hex!("750c31d2290c456fcca1c659b6add80e7a88f881")),
            pool: Address::new(hex!("848a5564158d84b8A8fb68ab5D004Fae11619A54")),
            token: vec![TokenInfoWithAddress {
                address: Address::new(hex!("cd5fe23c85820f7b72d0926fc9b05b43e359b7ee")),
                inner: TokenInfo { decimals: 18, symbol: "weETH".to_string() },
            }],
            amount: vec![U256::from_str("1935117712922949743")
                .unwrap()
                .to_scaled_rational(18)],
        });

        classifier_utils
            .contains_action(
                mint,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Action::is_mint),
            )
            .await
            .unwrap();
    }

    #[brontes_macros::test]
    async fn test_balancer_v2_exit_pool() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let burn =
            B256::from(hex!("ad13973ee8e507b36adc5d28dc53b77d58d00d5ac6a09aa677936be8aaf6c8a1"));

        classifier_utils.ensure_token(TokenInfoWithAddress {
            address: Address::new(hex!("bf5495efe5db9ce00f80364c8b423567e58d2110")),
            inner: TokenInfo { decimals: 18, symbol: "ezETH".to_string() },
        });

        classifier_utils.ensure_token(TokenInfoWithAddress {
            address: Address::new(hex!("cd5fe23c85820f7b72d0926fc9b05b43e359b7ee")),
            inner: TokenInfo { decimals: 18, symbol: "weETH".to_string() },
        });

        classifier_utils.ensure_token(TokenInfoWithAddress {
            address: Address::new(hex!("fae103dc9cf190ed75350761e95403b7b8afa6c0")),
            inner: TokenInfo { decimals: 18, symbol: "rswETH".to_string() },
        });

        let eq_action = Action::Burn(NormalizedBurn {
            protocol: Protocol::BalancerV2,
            trace_index: 0,
            from: Address::new(hex!("f4283d13ba1e17b33bb3310c3149136a2ef79ef7")),
            recipient: Address::new(hex!("f4283d13ba1e17b33bb3310c3149136a2ef79ef7")),
            pool: Address::new(hex!("848a5564158d84b8A8fb68ab5D004Fae11619A54")),
            token: vec![
                TokenInfoWithAddress {
                    address: Address::new(hex!("bf5495efe5db9ce00f80364c8b423567e58d2110")),
                    inner: TokenInfo { decimals: 18, symbol: "ezETH".to_string() },
                },
                TokenInfoWithAddress {
                    address: Address::new(hex!("cd5fe23c85820f7b72d0926fc9b05b43e359b7ee")),
                    inner: TokenInfo { decimals: 18, symbol: "weETH".to_string() },
                },
                TokenInfoWithAddress {
                    address: Address::new(hex!("fae103dc9cf190ed75350761e95403b7b8afa6c0")),
                    inner: TokenInfo { decimals: 18, symbol: "rswETH".to_string() },
                },
            ],
            amount: vec![
                U256::from_str("471937215318872937")
                    .unwrap()
                    .to_scaled_rational(18),
                U256::from_str("757823171697267931")
                    .unwrap()
                    .to_scaled_rational(18),
                U256::from_str("699970729674926490")
                    .unwrap()
                    .to_scaled_rational(18),
            ],
        });

        classifier_utils
            .contains_action(
                burn,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Action::is_burn),
            )
            .await
            .unwrap();
    }
}
