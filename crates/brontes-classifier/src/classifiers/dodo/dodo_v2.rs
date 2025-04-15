use alloy_primitives::U256;
use brontes_macros::action_impl;
use brontes_types::{
    normalized_actions::{NormalizedBurn, NormalizedFlashLoan, NormalizedMint, NormalizedSwap},
    structured_trace::CallInfo,
    Protocol, ToScaledRational,
};

action_impl!(
    Protocol::Dodo,
    crate::DodoDPPPool::sellBaseCall,
    Swap,
    [DODOSwap],
    logs: true,
    include_delegated_logs: true,
    |info: CallInfo, log_data: DodoSellBaseCallLogs, db: &DB| {
        let logs = log_data.d_o_d_o_swap_field?;

        let token_in = db.try_fetch_token_info(logs.fromToken)?;
        let token_out = db.try_fetch_token_info(logs.toToken)?;

        let amount_in = logs.fromAmount.to_scaled_rational(token_in.decimals);
        let amount_out = logs.toAmount.to_scaled_rational(token_out.decimals);

        Ok(NormalizedSwap {
            protocol: Protocol::Dodo,
            trace_index: info.trace_idx,
            from: logs.trader,
            recipient: logs.receiver,
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
    Protocol::Dodo,
    crate::DodoDPPPool::sellQuoteCall,
    Swap,
    [DODOSwap],
    logs: true,
    include_delegated_logs: true,
    |info: CallInfo, log_data: DodoSellQuoteCallLogs, db: &DB| {
        let logs = log_data.d_o_d_o_swap_field?;

        let token_in = db.try_fetch_token_info(logs.fromToken)?;
        let token_out = db.try_fetch_token_info(logs.toToken)?;

        let amount_in = logs.fromAmount.to_scaled_rational(token_in.decimals);
        let amount_out = logs.toAmount.to_scaled_rational(token_out.decimals);

        Ok(NormalizedSwap {
            protocol: Protocol::Dodo,
            trace_index: info.trace_idx,
            from: logs.trader,
            recipient: logs.receiver,
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
    Protocol::Dodo,
    crate::DodoDPPPool::flashLoanCall,
    FlashLoan,
    [DODOFlashLoan],
    logs: true,
    include_delegated_logs: true,
    |info: CallInfo, log_data: DodoFlashLoanCallLogs, db: &DB| {
        let logs = log_data.d_o_d_o_flash_loan_field?;

        let details = db.get_protocol_details_sorted(info.target_address)?;
        let [token_0, token_1] = [details.token0, details.token1];

        let token_one = db.try_fetch_token_info(token_0)?;
        let token_two = db.try_fetch_token_info(token_1)?;

        let amount_one = logs.baseAmount.to_scaled_rational(token_one.decimals);
        let amount_two = logs.quoteAmount.to_scaled_rational(token_two.decimals);

        Ok(NormalizedFlashLoan {
            protocol: Protocol::Dodo,
            trace_index: info.trace_idx,
            from: logs.borrower,
            pool: info.target_address,
            receiver_contract: logs.assetTo,
            assets: vec![token_one, token_two],
            amounts: vec![amount_one, amount_two],

            // Empty
            aave_mode: None,
            child_actions: vec![],
            repayments: vec![],
            fees_paid: vec![],
            msg_value: info.msg_value
        })
    }
);

action_impl!(
    Protocol::Dodo,
    crate::DodoDSPPool::buySharesCall,
    Mint,
    [],
    return_data: true,
    call_data: true,
    |info: CallInfo, call_data: buySharesCall, return_data: buySharesReturn, db: &DB| {
        let details = db.get_protocol_details_sorted(info.target_address)?;
        let [token_0, token_1] = [details.token0, details.token1];

        let mut token = vec![];
        let mut amount = vec![];

        if return_data.baseInput > U256::ZERO {
            let token_one = db.try_fetch_token_info(token_0)?;
            let amount_one = return_data.baseInput.to_scaled_rational(token_one.decimals);
            token.push(token_one);
            amount.push(amount_one);
        }

        if return_data.quoteInput > U256::ZERO {
            let token_two = db.try_fetch_token_info(token_1)?;
            let amount_two = return_data.quoteInput.to_scaled_rational(token_two.decimals);
            token.push(token_two);
            amount.push(amount_two);
        }

        Ok(NormalizedMint {
            protocol: Protocol::Dodo,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: call_data.to,
            pool: info.target_address,
            token,
            amount
        })
    }
);

action_impl!(
    Protocol::Dodo,
    crate::DodoDSPPool::sellSharesCall,
    Burn,
    [],
    call_data: true,
    return_data: true,
    |info: CallInfo, call_data: sellSharesCall, return_data: sellSharesReturn, db: &DB| {
        let details = db.get_protocol_details_sorted(info.target_address)?;
        let [token_0, token_1] = [details.token0, details.token1];

        let mut token = vec![];
        let mut amount = vec![];

        if return_data.baseAmount > U256::ZERO {
            let token_one = db.try_fetch_token_info(token_0)?;
            let amount_one = return_data.baseAmount.to_scaled_rational(token_one.decimals);
            token.push(token_one);
            amount.push(amount_one);
        }

        if return_data.quoteAmount > U256::ZERO {
            let token_two = db.try_fetch_token_info(token_1)?;
            let amount_two = return_data.quoteAmount.to_scaled_rational(token_two.decimals);
            token.push(token_two);
            amount.push(amount_two);
        }

        Ok(NormalizedBurn {
            protocol: Protocol::Dodo,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: call_data.to,
            pool: info.target_address,
            token,
            amount
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
    async fn test_dodo_buy_shares() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let mint =
            B256::from(hex!("88f60c94b868a5558bc53268ec035ffbf482381bbbeafdbdc03adaff11911e69"));

        let token = vec![
            TokenInfoWithAddress {
                address: Address::new(hex!("888f538aa0634472d3f038f225c59b5847cde015")),
                inner:   TokenInfo { decimals: 18, symbol: "NGN".to_string() },
            },
            TokenInfoWithAddress::weth(),
        ];

        classifier_utils.ensure_token(token[0].clone());

        classifier_utils.ensure_protocol(
            Protocol::Dodo,
            hex!("57dAe55C697929FFB920942ad25b10908edDc56E").into(),
            WETH_ADDRESS,
            Some(hex!("888f538aa0634472d3f038f225c59b5847cde015").into()),
            None,
            None,
            None,
            None,
        );

        let eq_action = Action::Mint(NormalizedMint {
            protocol: Protocol::Dodo,
            trace_index: 13,
            from: Address::new(hex!("a356867fdcea8e71aeaf87805808803806231fdc")),
            recipient: Address::new(hex!("79b7b57df6422dd1b690cfaeac6fc61095f179a3")),
            pool: Address::new(hex!("57dae55c697929ffb920942ad25b10908eddc56e")),
            token,
            amount: vec![
                U256::from_str("704897023978838744")
                    .unwrap()
                    .to_scaled_rational(18),
                U256::from_str("1495827155313454289417")
                    .unwrap()
                    .to_scaled_rational(18),
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
    async fn test_dodo_sell_shares() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let mint =
            B256::from(hex!("346eb129f70aecb9ca1a2b3cbcb3cabd2d1cd5fec46c91fff41d4257114148e9"));

        let token = vec![
            TokenInfoWithAddress {
                address: Address::new(hex!("9bf1d7d63dd7a4ce167cf4866388226eeefa702e")),
                inner:   TokenInfo { decimals: 18, symbol: "BEN".to_string() },
            },
            TokenInfoWithAddress::weth(),
        ];

        classifier_utils.ensure_token(token[0].clone());

        classifier_utils.ensure_protocol(
            Protocol::Dodo,
            hex!("6AE6D8264A533DE49Dad16bee09761EA97b559Cd").into(),
            WETH_ADDRESS,
            Some(hex!("9bf1d7d63dd7a4ce167cf4866388226eeefa702e").into()),
            None,
            None,
            None,
            None,
        );

        let eq_action = Action::Burn(NormalizedBurn {
            protocol: Protocol::Dodo,
            trace_index: 0,
            from: Address::new(hex!("e2752B80FF0322f8E370625B645929D2BB21F26f")),
            recipient: Address::new(hex!("e2752B80FF0322f8E370625B645929D2BB21F26f")),
            pool: Address::new(hex!("6AE6D8264A533DE49Dad16bee09761EA97b559Cd")),
            token,
            amount: vec![
                U256::from_str("374423721407789417")
                    .unwrap()
                    .to_scaled_rational(18),
                U256::from_str("93742650560116459442507612342")
                    .unwrap()
                    .to_scaled_rational(18),
            ],
        });

        classifier_utils
            .contains_action(
                mint,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Action::is_burn),
            )
            .await
            .unwrap();
    }

    // Tested but couldn't find a transaction that was less than 300 lines of
    // trace. #[brontes_macros::test]
    // async fn test_dodo_sell_base() {
    //     let classifier_utils = ClassifierTestUtils::new().await;
    //     let swap =
    //         B256::from(hex!("
    // 408a31b29abd74fd5cef887e7770c230cb881f4928b037149f4877a9aa8edf9d"));

    //     let token_in = TokenInfoWithAddress::weth();
    //     let token_out = TokenInfoWithAddress {
    //         address:
    // Address::new(hex!("9bf1d7d63dd7a4ce167cf4866388226eeefa702e")),
    //         inner:   TokenInfo { decimals: 18, symbol: "BEN".to_string() },
    //     };

    //     classifier_utils.ensure_token(token_out.clone());

    //     classifier_utils.ensure_protocol(
    //         Protocol::Dodo,
    //         hex!("6AE6D8264A533DE49Dad16bee09761EA97b559Cd").into(),
    //         WETH_ADDRESS,
    //         Some(hex!("9bf1d7d63dd7a4ce167cf4866388226eeefa702e").into()),
    //         None,
    //         None,
    //         None,
    //         None,
    //     );

    //     // sell Base
    //     let eq_action = Actions::Swap(NormalizedSwap {
    //         protocol:    Protocol::Dodo,
    //         trace_index: 1,
    //         from:
    // Address::new(hex!("E37e799D5077682FA0a244D46E5649F71457BD09")),
    //         recipient:
    // Address::new(hex!("1111111254EEB25477B68fb85Ed929f73A960582")),
    //         pool:
    // Address::new(hex!("358e056c50eea4ca707e891404e81d9b898d0b41")),
    //         token_in,
    //         amount_in:   U256::from_str("1702990445757351")
    //             .unwrap()
    //             .to_scaled_rational(18),
    //         token_out,
    //         amount_out:  U256::from_str("471051074073405078175435521")
    //             .unwrap()
    //             .to_scaled_rational(18),
    //         msg_value: U256::ZERO,
    //     });

    //     classifier_utils
    //         .contains_action(
    //             swap,
    //             0,
    //             eq_action,
    //             TreeSearchBuilder::default().with_action(Actions::is_swap),
    //         )
    //         .await
    //         .unwrap();
    // }
}
