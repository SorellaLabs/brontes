use brontes_macros::action_impl;
use brontes_types::{
    normalized_actions::{NormalizedMint, NormalizedSwap},
    structured_trace::CallInfo,
    Protocol, ToScaledRational,
};

action_impl!(
    Protocol::BancorV3,
    crate::BancorNetwork::tradeBySourceAmountCall,
    Swap,
    [TokensTraded],
    logs: true,
    include_delegated_logs: true,
    |call_info: CallInfo, logs: BancorV3TradeBySourceAmountCallLogs, db_tx: &DB| {
        let log = logs.tokens_traded_field?;

        let token_in = db_tx.try_fetch_token_info(log.sourceToken)?;
        let token_out = db_tx.try_fetch_token_info(log.targetToken)?;

        let amount_in = log.sourceAmount.to_scaled_rational(token_in.decimals);
        let amount_out = log.targetAmount.to_scaled_rational(token_out.decimals);

        Ok(NormalizedSwap {
            protocol: Protocol::BancorV3,
            trace_index: call_info.trace_idx,
            pool: call_info.target_address,
            from: log.trader,
            recipient: log.trader,
            token_in,
            token_out,
            amount_in,
            amount_out,
            msg_value: call_info.msg_value
        })
    }
);

action_impl!(
    Protocol::BancorV3,
    crate::BancorNetwork::tradeByTargetAmountCall,
    Swap,
    [TokensTraded],
    logs: true,
    include_delegated_logs: true,
    |call_info: CallInfo, logs: BancorV3TradeByTargetAmountCallLogs, db_tx: &DB| {
        let log = logs.tokens_traded_field?;

        let token_in = db_tx.try_fetch_token_info(log.sourceToken)?;
        let token_out = db_tx.try_fetch_token_info(log.targetToken)?;

        let amount_in = log.sourceAmount.to_scaled_rational(token_in.decimals);
        let amount_out = log.targetAmount.to_scaled_rational(token_out.decimals);

        Ok(NormalizedSwap {
            protocol: Protocol::BancorV3,
            trace_index: call_info.trace_idx,
            pool: call_info.target_address,
            from: log.trader,
            recipient: log.trader,
            token_in,
            token_out,
            amount_in,
            amount_out,
            msg_value: call_info.msg_value
        })
    }
);

action_impl!(
    Protocol::BancorV3,
    crate::BancorNetwork::depositCall,
    Mint,
    [],
    call_data: true,
    include_delegated_logs: true,
    |call_info: CallInfo, call_data: depositCall, db_tx: &DB| {
        let token = db_tx.try_fetch_token_info(call_data.pool)?;
        let amount = call_data.tokenAmount.to_scaled_rational(token.decimals);

        Ok(NormalizedMint {
            protocol: Protocol::BancorV3,
            trace_index: call_info.trace_idx,
            pool: call_info.target_address,
            from: call_info.from_address,
            recipient: call_info.from_address,
            token: vec![token],
            amount: vec![amount]
        })
    }
);

action_impl!(
    Protocol::BancorV3,
    crate::BancorNetwork::depositForCall,
    Mint,
    [],
    call_data: true,
    include_delegated_logs: true,
    |call_info: CallInfo, call_data: depositForCall, db_tx: &DB| {
        let token = db_tx.try_fetch_token_info(call_data.pool)?;
        let amount = call_data.tokenAmount.to_scaled_rational(token.decimals);

        Ok(NormalizedMint {
            protocol: Protocol::BancorV3,
            trace_index: call_info.trace_idx,
            pool: call_info.target_address,
            from: call_info.from_address,
            recipient: call_data.provider,
            token: vec![token],
            amount: vec![amount]
        })
    }
);

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{hex, Address, B256, U256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        db::token_info::{TokenInfo, TokenInfoWithAddress},
        normalized_actions::Actions,
        Protocol::BancorV3,
        TreeSearchBuilder,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_bancor_v3_trade_by_source_amount() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let swap =
            B256::from(hex!("558114a176022f0d8fd70303291e5e5f903f52023797841f7afc9d414aea80fb"));

        let token_in = TokenInfoWithAddress::weth();
        let token_out = TokenInfoWithAddress {
            address: Address::new(hex!("1F573D6Fb3F13d689FF844B4cE37794d79a7FF1C")),
            inner:   TokenInfo { symbol: "BNT".to_string(), decimals: 18 },
        };

        let amount_in = U256::from_str("3000000000000000000")
            .unwrap()
            .to_scaled_rational(18);
        let amount_out = U256::from_str("11025577292695098102559")
            .unwrap()
            .to_scaled_rational(18);

        classifier_utils.ensure_token(token_out.clone());

        let eq_action = Actions::Swap(NormalizedSwap {
            protocol: BancorV3,
            trace_index: 0,
            from: Address::new(hex!("279DDbe45D9c34025DD38F627Bc4C6dc92BC665A")),
            recipient: Address::new(hex!("279DDbe45D9c34025DD38F627Bc4C6dc92BC665A")),
            pool: Address::new(hex!("eEF417e1D5CC832e619ae18D2F140De2999dD4fB")),
            token_in,
            amount_in,
            token_out,
            amount_out,
            msg_value: U256::from_str("3000000000000000000").unwrap(),
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
    async fn test_bancor_v3_trade_by_target_amount() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let swap =
            B256::from(hex!("ab1d20cffc3a74d0f07342bab8bb60f3f510d42e984993e8e97c46fec978ccca"));

        let token_in = TokenInfoWithAddress::weth();
        let token_out = TokenInfoWithAddress {
            address: Address::new(hex!("b9ef770b6a5e12e45983c5d80545258aa38f3b78")),
            inner:   TokenInfo { symbol: "ZCN".to_string(), decimals: 10 },
        };

        let amount_in = U256::from_str("753504506848492")
            .unwrap()
            .to_scaled_rational(18);
        let amount_out = U256::from_str("100000000000")
            .unwrap()
            .to_scaled_rational(10);

        classifier_utils.ensure_token(token_out.clone());

        let eq_action = Actions::Swap(NormalizedSwap {
            protocol: BancorV3,
            trace_index: 0,
            from: Address::new(hex!("1B0ad245b944EEF1Bd647276b7f407277c700654")),
            recipient: Address::new(hex!("1B0ad245b944EEF1Bd647276b7f407277c700654")),
            pool: Address::new(hex!("eEF417e1D5CC832e619ae18D2F140De2999dD4fB")),
            token_in,
            amount_in,
            token_out,
            amount_out,
            msg_value: U256::from_str("1334598240000000").unwrap(),
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
}
