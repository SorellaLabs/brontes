use brontes_macros::action_impl;
use brontes_types::{
    normalized_actions::{NormalizedBurn, NormalizedMint, NormalizedSwap},
    structured_trace::CallInfo,
    Protocol, ToScaledRational,
};
use reth_primitives::U256;

action_impl!(
    Protocol::MaverickV2,
    crate::MaverickV2Pool::swapCall,
    Swap,
    [PoolSwap],
    logs: true,
    |info: CallInfo, log_data: MaverickV2SwapCallLogs, db_tx: &DB| {
        let logs = log_data.pool_swap_field?;

        let details=db_tx.get_protocol_details(info.from_address)?;

        let token_in_addr = if logs.params.tokenAIn {
            details.token0
        } else {
            details.token1
        };

        let token_out_addr = if logs.params.tokenAIn {
            details.token1
        } else {
            details.token0
        };

        let token_in = db_tx.try_fetch_token_info(token_in_addr)?;
        let token_out = db_tx.try_fetch_token_info(token_out_addr)?;

        let amount_in = logs.amountIn.to_scaled_rational(token_in.decimals);
        let amount_out = logs.amountOut.to_scaled_rational(token_out.decimals);

        Ok(NormalizedSwap {
            protocol: Protocol::MaverickV2,
            trace_index: info.trace_idx,
            from: logs.sender,
            recipient: logs.recipient,
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
    Protocol::MaverickV2,
    crate::MaverickV2Pool::addLiquidityCall,
    Mint,
    [PoolAddLiquidity],
    return_data: true,
    logs: true,
    call_data: true,
    |
     info: CallInfo,
     call_data: addLiquidityCall,
     return_data: addLiquidityReturn, _logs: MaverickV2AddLiquidityCallLogs,  db_tx: &DB| {

        let token_0_delta=return_data.tokenAAmount;
        let token_1_delta=return_data.tokenBAmount;

        let details=db_tx.get_protocol_details_sorted(info.target_address)?;
        let [token_0, token_1]=[details.token0, details.token1];

        let t0_info=db_tx.try_fetch_token_info(token_0)?;
        let t1_info=db_tx.try_fetch_token_info(token_1)?;

        let am0=token_0_delta.to_scaled_rational(t0_info.decimals);
        let am1=token_1_delta.to_scaled_rational(t1_info.decimals);
        Ok(NormalizedMint {
            protocol: Protocol::MaverickV2,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: call_data.recipient,
            pool: info.target_address,
            token: vec![t0_info, t1_info],
            amount: vec![am0, am1],
        })
    }
);

action_impl!(
    Protocol::MaverickV2,
    crate::MaverickV2Pool::removeLiquidityCall,
    Burn,
    [PoolRemoveLiquidity],
    return_data: true,
    |
    info: CallInfo,
    return_data: removeLiquidityReturn,
    db_tx: &DB| {
        let token_0_delta: U256 = return_data.tokenAOut;
        let token_1_delta: U256 = return_data.tokenBOut;
        let details = db_tx.get_protocol_details_sorted(info.target_address)?;
        let [token_0, token_1] = [details.token0, details.token1];

        let t0_info = db_tx.try_fetch_token_info(token_0)?;
        let t1_info = db_tx.try_fetch_token_info(token_1)?;

        let am0 = token_0_delta.to_scaled_rational(t0_info.decimals);
        let am1 = token_1_delta.to_scaled_rational(t1_info.decimals);

        Ok(NormalizedBurn {
            protocol: Protocol::MaverickV2,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: info.from_address,
            pool: info.target_address,
            token: vec![t0_info, t1_info],
            amount: vec![am0, am1],
        })
    }
);

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{hex, Address, B256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        constants::WETH_ADDRESS,
        db::token_info::{TokenInfo, TokenInfoWithAddress},
        normalized_actions::Action,
        TreeSearchBuilder,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_maverick_v2_swap() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let swap =
            B256::from(hex!("88f60c94b868a5558bc53268ec035ffbf482381bbbeafdbdc03adaff11911e69"));

        let tokens = vec![
            TokenInfoWithAddress {
                address: Address::new(hex!("888f538aa0634472d3f038f225c59b5847cde015")),
                inner:   TokenInfo { decimals: 18, symbol: "NGN".to_string() },
            },
            TokenInfoWithAddress::weth(),
        ];

        classifier_utils.ensure_token(tokens[0].clone());

        classifier_utils.ensure_protocol(
            Protocol::MaverickV2,
            hex!("57dAe55C697929FFB920942ad25b10908edDc56E").into(),
            WETH_ADDRESS,
            Some(hex!("888f538aa0634472d3f038f225c59b5847cde015").into()),
            None,
            None,
            None,
            None,
        );

        let eq_action = Action::Swap(NormalizedSwap {
            protocol:    Protocol::MaverickV2,
            trace_index: 13,
            from:        Address::new(hex!("a356867fdcea8e71aeaf87805808803806231fdc")),
            recipient:   Address::new(hex!("79b7b57df6422dd1b690cfaeac6fc61095f179a3")),
            pool:        Address::new(hex!("57dae55c697929ffb920942ad25b10908eddc56e")),
            token_in:    tokens[0].clone(),
            token_out:   tokens[1].clone(),
            amount_in:   U256::from_str("704897023978838744")
                .unwrap()
                .to_scaled_rational(18),
            amount_out:  U256::from_str("704897023978838744")
                .unwrap()
                .to_scaled_rational(18),
            msg_value:   U256::ZERO,
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
    async fn test_maverick_v2_mints() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let mint =
            B256::from(hex!("0089210683170b3f17201c8abeafdc4c022a26c7af1e44d351556eaa48d0fee8"));

        let eq_action = Action::Mint(NormalizedMint {
            protocol:    Protocol::MaverickV2,
            trace_index: 21,
            from:        Address::new(hex!("6b75d8AF000000e20B7a7DDf000Ba900b4009A80")),
            recipient:   Address::new(hex!("6b75d8AF000000e20B7a7DDf000Ba900b4009A80")),
            pool:        Address::new(hex!("3416cF6C708Da44DB2624D63ea0AAef7113527C6")),
            token:       vec![TokenInfoWithAddress::usdc(), TokenInfoWithAddress::usdt()],
            amount:      vec![
                U256::from_str("102642322850")
                    .unwrap()
                    .to_scaled_rational(6),
                U256::from_str("250137480130")
                    .unwrap()
                    .to_scaled_rational(6),
            ],
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
    async fn test_maverick_v2_burn() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let burn =
            B256::from(hex!("f179f349434a59d0dc899fc03a5754c7e50f52de1709d9523e7cbd09c4ba13eb"));

        let eq_action = Action::Burn(NormalizedBurn {
            protocol:    Protocol::MaverickV2,
            trace_index: 12,
            from:        Address::new(hex!("6b75d8AF000000e20B7a7DDf000Ba900b4009A80")),
            recipient:   Address::new(hex!("6b75d8AF000000e20B7a7DDf000Ba900b4009A80")),
            pool:        Address::new(hex!("3416cF6C708Da44DB2624D63ea0AAef7113527C6")),
            token:       vec![TokenInfoWithAddress::usdc(), TokenInfoWithAddress::usdt()],
            amount:      vec![
                U256::from_str("347057356182")
                    .unwrap()
                    .to_scaled_rational(6),
                U256::from_str("5793599811").unwrap().to_scaled_rational(6),
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
