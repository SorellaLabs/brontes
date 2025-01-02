use brontes_macros::action_impl;
use brontes_types::{
    normalized_actions::NormalizedSwap, structured_trace::CallInfo, Protocol, ToScaledRational,
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

        classifier_utils.ensure_token(token[0].clone());

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
            amount_in:   vec![U256::from_str("704897023978838744")
                .unwrap()
                .to_scaled_rational(18)],
            amount_out:  vec![U256::from_str("704897023978838744")
                .unwrap()
                .to_scaled_rational(18)],
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
}
