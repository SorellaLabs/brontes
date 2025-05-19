use brontes_database::libmdbx::LibmdbxReader;
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::{
        NormalizedBurn, NormalizedMint, NormalizedNewPool, NormalizedSwap,
    },
    structured_trace::CallInfo,
    ToScaledRational,
};

action_impl!(
    Protocol::BalancerV3,
    crate::BalancerV3Vault::swapCall,
    Swap,
    [..],
    call_data: true,
    return_data:true,
    |info: CallInfo, call_data: swapCall, return_data: swapReturn, db: &DB| {

        let vault_swap_params=call_data.vaultSwapParams;
        let pool = vault_swap_params.pool;
        let token_in = db.try_fetch_token_info(vault_swap_params.tokenIn)?;
        let token_out = db.try_fetch_token_info(vault_swap_params.tokenOut)?;
        let amount_in = return_data.amountIn.to_scaled_rational(token_in.decimals);
        let amount_out = return_data.amountOut.to_scaled_rational(token_out.decimals);

        Ok(NormalizedSwap {
            protocol: Protocol::BalancerV3,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: info.msg_sender,
            pool,
            token_in,
            amount_in,
            token_out,
            amount_out,
            msg_value: info.msg_value,
        })
    }
);

action_impl!(
    Protocol::BalancerV3,
    crate::BalancerV3Vault::addLiquidityCall,
    Mint,
    [..],
    call_data: true,
    return_data: true,
    |info: CallInfo, call_data: addLiquidityCall, return_data: addLiquidityReturn, db: &DB| {
        let call_params=call_data.params;
        let pool=call_params.pool;
        let recipient=call_params.to;
        let details=db.get_protocol_details(pool)?;
        let tokens=details.get_tokens();
        let tokens=tokens.iter().map(|token| db.try_fetch_token_info(*token)).collect::<Result<Vec<_>, _>>()?;

        let amounts=return_data.amountsIn.iter().zip(tokens.iter()).map(|(amount, token)| amount.to_scaled_rational(token.decimals)).collect();

        Ok(NormalizedMint {
            protocol: Protocol::BalancerV3,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient,
            pool,
            token:tokens,
            amount: amounts
        })
    }
);


action_impl!(
    Protocol::BalancerV3,
    crate::BalancerV3Vault::removeLiquidityCall,
    Burn,
    [..],
    call_data: true,
    return_data: true,
    |info: CallInfo, call_data: removeLiquidityCall,return_data: removeLiquidityReturn, db: &DB| {
        let call_params=call_data.params;
        let pool=call_params.pool;
        let recipient=call_params.from;
        let details=db.get_protocol_details(pool)?;
        let tokens=details.get_tokens();
        let tokens=tokens.iter().map(|token| db.try_fetch_token_info(*token)).collect::<Result<Vec<_>, _>>()?;

        let amounts=return_data.amountsOut.iter().zip(tokens.iter()).map(|(amount, token)| amount.to_scaled_rational(token.decimals)).collect();

        Ok(NormalizedBurn {
            protocol: Protocol::BalancerV3,
            trace_index: info.trace_idx,
            from: recipient,
            recipient,
            pool: call_params.pool,
            token: tokens,
            amount: amounts
        })
    }
);


action_impl!(
    Protocol::BalancerV3,
    crate::BalancerV3VaultExtension::registerPoolCall,
    NewPool,
    [..PoolRegistered],
    call_data:true,
    logs: true,
    |info: CallInfo, call_data: registerPoolCall, log_data: BalancerV3RegisterPoolCallLogs, _| {
        let logs = log_data.pool_registered_field?;
        let pool_address=call_data.pool;
        let tokens=logs.tokenConfig.iter().map(|token_config| token_config.token).collect::<Vec<_>>();

        Ok(NormalizedNewPool {
            trace_index: info.trace_idx,
            protocol: Protocol::BalancerV3,
            pool_address,
            tokens,
        })
    }
);


// ~ https://docs.balancer.fi/reference/contracts/pool-interfacing.html#poolids
// The poolId is a unique identifier, the first portion of which is the pool's
// contract address. For example, the pool with the id
// 0x5c6ee304399dbdb9c8ef030ab642b10820db8f56000200000000000000000014 has a
// contract address of 0x5c6ee304399dbdb9c8ef030ab642b10820db8f56.


#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{hex, B256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        constants::WETH_ADDRESS, db::token_info::TokenInfo, normalized_actions::Action,
        Protocol::BalancerV3, TreeSearchBuilder,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_balancer_v3_swap() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let swap =
            B256::from(hex!("da10a5e3cb8c34c77634cb9a1cfe02ec2b23029f1f288d79b6252b2f8cae20d3"));

        classifier_utils.ensure_token(TokenInfoWithAddress {
            address: Address::new(hex!("6C22910c6F75F828B305e57c6a54855D8adeAbf8")),
            inner:   TokenInfo { decimals: 9, symbol: "SATS".to_string() },
        });

        classifier_utils.ensure_protocol(
            Protocol::BalancerV3,
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
            protocol:    BalancerV3,
            trace_index: 1,
            from:        Address::new(hex!("5d2146eAB0C6360B864124A99BD58808a3014b5d")),
            recipient:   Address::new(hex!("5d2146eAB0C6360B864124A99BD58808a3014b5d")),
            pool:        Address::new(hex!("358e056c50eea4ca707e891404e81d9b898d0b41")),
            token_in:    TokenInfoWithAddress::weth(),
            amount_in:   U256::from_str("10000000000000000")
                .unwrap()
                .to_scaled_rational(18),
            token_out:   TokenInfoWithAddress {
                address: Address::new(hex!("6C22910c6F75F828B305e57c6a54855D8adeAbf8")),
                inner:   TokenInfo { decimals: 9, symbol: "SATS".to_string() },
            },
            amount_out:  U256::from_str("7727102831493")
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

    
    #[brontes_macros::test]
    async fn test_balancer_v3_join_pool() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let mint =
            B256::from(hex!("ffed34d6f2d9e239b5cd3985840a37f1fa0c558edcd1a2f3d2b8bd7f314ef6a3"));

        classifier_utils.ensure_token(TokenInfoWithAddress {
            address: Address::new(hex!("cd5fe23c85820f7b72d0926fc9b05b43e359b7ee")),
            inner:   TokenInfo { decimals: 18, symbol: "weETH".to_string() },
        });

        let eq_action = Action::Mint(NormalizedMint {
            protocol:    Protocol::BalancerV3,
            trace_index: 0,
            from:        Address::new(hex!("750c31d2290c456fcca1c659b6add80e7a88f881")),
            recipient:   Address::new(hex!("750c31d2290c456fcca1c659b6add80e7a88f881")),
            pool:        Address::new(hex!("848a5564158d84b8A8fb68ab5D004Fae11619A54")),
            token:       vec![TokenInfoWithAddress {
                address: Address::new(hex!("cd5fe23c85820f7b72d0926fc9b05b43e359b7ee")),
                inner:   TokenInfo { decimals: 18, symbol: "weETH".to_string() },
            }],
            amount:      vec![U256::from_str("1935117712922949743")
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
    async fn test_balancer_v3_exit_pool() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let burn =
            B256::from(hex!("ad13973ee8e507b36adc5d28dc53b77d58d00d5ac6a09aa677936be8aaf6c8a1"));

        classifier_utils.ensure_token(TokenInfoWithAddress {
            address: Address::new(hex!("bf5495efe5db9ce00f80364c8b423567e58d2110")),
            inner:   TokenInfo { decimals: 18, symbol: "ezETH".to_string() },
        });

        classifier_utils.ensure_token(TokenInfoWithAddress {
            address: Address::new(hex!("cd5fe23c85820f7b72d0926fc9b05b43e359b7ee")),
            inner:   TokenInfo { decimals: 18, symbol: "weETH".to_string() },
        });

        classifier_utils.ensure_token(TokenInfoWithAddress {
            address: Address::new(hex!("fae103dc9cf190ed75350761e95403b7b8afa6c0")),
            inner:   TokenInfo { decimals: 18, symbol: "rswETH".to_string() },
        });

        let eq_action = Action::Burn(NormalizedBurn {
            protocol:    Protocol::BalancerV3,
            trace_index: 0,
            from:        Address::new(hex!("f4283d13ba1e17b33bb3310c3149136a2ef79ef7")),
            recipient:   Address::new(hex!("f4283d13ba1e17b33bb3310c3149136a2ef79ef7")),
            pool:        Address::new(hex!("848a5564158d84b8A8fb68ab5D004Fae11619A54")),
            token:       vec![
                TokenInfoWithAddress {
                    address: Address::new(hex!("bf5495efe5db9ce00f80364c8b423567e58d2110")),
                    inner:   TokenInfo { decimals: 18, symbol: "ezETH".to_string() },
                },
                TokenInfoWithAddress {
                    address: Address::new(hex!("cd5fe23c85820f7b72d0926fc9b05b43e359b7ee")),
                    inner:   TokenInfo { decimals: 18, symbol: "weETH".to_string() },
                },
                TokenInfoWithAddress {
                    address: Address::new(hex!("fae103dc9cf190ed75350761e95403b7b8afa6c0")),
                    inner:   TokenInfo { decimals: 18, symbol: "rswETH".to_string() },
                },
            ],
            amount:      vec![
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
