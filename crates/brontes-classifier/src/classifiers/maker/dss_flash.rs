use brontes_macros::action_impl;
use brontes_types::{
    normalized_actions::NormalizedFlashLoan, structured_trace::CallInfo, Protocol, ToScaledRational,
};

action_impl!(
    Protocol::MakerDssFlash,
    crate::MakerDssFlash::flashLoanCall,
    FlashLoan,
    [FlashLoan],
    call_data: true,
    logs: true,
    |call_info: CallInfo,
    call_data: flashLoanCall,
    log_data: MakerDssFlashFlashLoanCallLogs,
    db_tx: &DB| {
        let logs = log_data.flash_loan_field?;

        let token = db_tx.try_fetch_token_info(call_data.token)?;
        let amount = call_data.amount.to_scaled_rational(token.decimals);

        Ok(NormalizedFlashLoan {
            protocol: Protocol::MakerDssFlash,
            trace_index: call_info.trace_idx,
            from: call_info.from_address,
            pool: call_info.target_address,
            msg_value: call_info.msg_value,
            receiver_contract: logs.receiver,
            assets: vec![token],
            amounts: vec![amount],

            // Empty
            aave_mode: None,
            child_actions: vec![],
            repayments: vec![],
            fees_paid: vec![],
        })
    }
);

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{hex, Address, B256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        db::token_info::{TokenInfo, TokenInfoWithAddress},
        normalized_actions::{Action, NormalizedTransfer},
        TreeSearchBuilder,
    };
    use alloy_primitives::U256;

    use super::*;

    #[brontes_macros::test]
    async fn test_maker_dss_flashloan() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let flashloan_tx =
            B256::from(hex!("8e2d6af376182807f0671f1504767c7723c49921344ce4f5799d8ba2d30d014c"));

        let dai = TokenInfoWithAddress {
            address: Address::new(hex!("6b175474e89094c44da98b954eedeac495271d0f")),
            inner:   TokenInfo { decimals: 18, symbol: "DAI".to_string() },
        };

        let eq_action = Action::FlashLoan(NormalizedFlashLoan {
            protocol:          Protocol::MakerDssFlash,
            trace_index:       2,
            from:              Address::new(hex!("1aecea38b8626eeb3748234343cff427268dd487")),
            pool:              Address::new(hex!("60744434d6339a6b27d73d9eda62b6f66a0a04fa")),
            receiver_contract: Address::new(hex!("1aecea38b8626eeb3748234343cff427268dd487")),
            assets:            vec![dai.clone()],
            amounts:           vec![U256::from_str("100000000").unwrap().to_scaled_rational(0)],
            aave_mode:         None,
            // Ignore child actions as we only need to focus on pruning necessary nodes.
            child_actions:     vec![],
            repayments:        vec![NormalizedTransfer {
                msg_value:   U256::ZERO,
                trace_index: 238,
                from:        Address::new(hex!("1aecea38b8626eeb3748234343cff427268dd487")),
                to:          Address::new(hex!("60744434d6339a6b27d73d9eda62b6f66a0a04fa")),
                token:       dai,
                amount:      U256::from_str("100000000").unwrap().to_scaled_rational(0),
                fee:         U256::ZERO.to_scaled_rational(0),
            }],
            fees_paid:         vec![],
            msg_value:         U256::ZERO,
        });

        classifier_utils
            .contains_action_except(
                flashloan_tx,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Action::is_flash_loan),
                &["child_actions"],
            )
            .await
            .unwrap();
    }
}
