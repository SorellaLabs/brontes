use alloy_primitives::{hex, Address};
use brontes_macros::action_impl;
use brontes_types::{
    normalized_actions::{NormalizedFlashLoan, NormalizedMint, NormalizedSwap},
    structured_trace::CallInfo,
    Protocol, ToScaledRational,
};

pub const BANCOR_V3_MASTER_VAULT: Address = Address::new(hex!(
    "649765821D9f64198c905eC0B2B037a4a52Bc373
    "
));

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
            pool: BANCOR_V3_MASTER_VAULT,
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
            pool: BANCOR_V3_MASTER_VAULT,
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
            pool: BANCOR_V3_MASTER_VAULT,
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
            pool: BANCOR_V3_MASTER_VAULT,
            from: call_info.from_address,
            recipient: call_data.provider,
            token: vec![token],
            amount: vec![amount]
        })
    }
);

action_impl!(
    Protocol::BancorV3,
    crate::BancorNetwork::flashLoanCall,
    FlashLoan,
    [],
    call_data: true,
    include_delegated_logs: true,
    |call_info: CallInfo, call_data: flashLoanCall, db_tx: &DB| {
        let token = db_tx.try_fetch_token_info(call_data.token)?;
        let amount = call_data.amount.to_scaled_rational(token.decimals);

        Ok(NormalizedFlashLoan {
            protocol: Protocol::BancorV3,
            trace_index: call_info.trace_idx,
            pool: BANCOR_V3_MASTER_VAULT,
            from: call_info.from_address,
            receiver_contract: call_data.recipient,
            aave_mode: None,
            assets: vec![token],
            amounts: vec![amount],
            child_actions: vec![],
            fees_paid: vec![],
            repayments: vec![],

            msg_value: call_info.msg_value
        })
    }
);

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{B256, U256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        db::token_info::{TokenInfo, TokenInfoWithAddress},
        normalized_actions::{Actions, NormalizedEthTransfer, Repayment},
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
            pool: BANCOR_V3_MASTER_VAULT,
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
            pool: BANCOR_V3_MASTER_VAULT,
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

    #[brontes_macros::test]
    async fn test_bancor_v3_flash_loan() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let flash_loan =
            B256::from(hex!("84b5586863e52e9f70b0ed8e7d832fc39e25ca7ea28e7a9aa4220587ddd682d3"));

        let eq_action = Actions::FlashLoan(NormalizedFlashLoan {
            protocol:          Protocol::BancorV3,
            trace_index:       2,
            from:              Address::new(hex!("41eeba3355d7d6ff628b7982f3f9d055c39488cb")),
            pool:              BANCOR_V3_MASTER_VAULT,
            receiver_contract: Address::new(hex!("41eeba3355d7d6ff628b7982f3f9d055c39488cb")),
            assets:            vec![TokenInfoWithAddress::weth()],
            amounts:           vec![U256::from_str("4387616000000000000")
                .unwrap()
                .to_scaled_rational(18)],
            aave_mode:         None,
            child_actions:     vec![],
            repayments:        vec![Repayment::Eth(NormalizedEthTransfer {
                trace_index:       18,
                from:              Address::new(hex!("eef417e1d5cc832e619ae18d2f140de2999dd4fb")),
                to:                Address::new(hex!("649765821d9f64198c905ec0b2b037a4a52bc373")),
                value:             U256::from_str("4387616000000000000").unwrap(),
                coinbase_transfer: false,
            })],
            fees_paid:         vec![],
            msg_value:         U256::ZERO,
        });

        classifier_utils
            .contains_action_except(
                flash_loan,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Actions::is_flash_loan),
                &["child_actions"],
            )
            .await
            .unwrap();
    }
}
