use alloy_primitives::U256;
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::NormalizedSwap, structured_trace::CallInfo, ToScaledRational,
};

// Native Orders
action_impl!(
    Protocol::Dexalot,
    crate::Dexalot::simpleSwapCall,
    Swap,
    [SwapExecuted],
    logs: true,
    include_delegated_logs: true,
    |info: CallInfo, logs: DexalotSimpleSwapCallLogs, db: &DB| {
        let logs = logs.swap_executed_field?;

        let token_in = db.try_fetch_token_info(logs.takerAsset)?;
        let token_out = db.try_fetch_token_info(logs.makerAsset)?;

        let amount_in = U256::from(logs.takerAmountReceived)
            .to_scaled_rational(token_in.decimals);
        let amount_out = U256::from(logs.makerAmountReceived)
            .to_scaled_rational(token_out.decimals);

        Ok(NormalizedSwap {
            protocol: Protocol::Dexalot,
            trace_index: info.trace_idx,
            from: logs.taker,
            recipient: logs.taker,
            msg_value: info.msg_value,
            pool: info.target_address,
            token_in,
            token_out,
            amount_in,
            amount_out
        })
    }
);

action_impl!(
    Protocol::Dexalot,
    crate::Dexalot::partialSwapCall,
    Swap,
    [SwapExecuted],
    logs: true,
    include_delegated_logs: true,
    |info: CallInfo, logs: DexalotPartialSwapCallLogs, db: &DB| {
        let logs = logs.swap_executed_field?;

        let token_in = db.try_fetch_token_info(logs.takerAsset)?;
        let token_out = db.try_fetch_token_info(logs.makerAsset)?;

        let amount_in = U256::from(logs.takerAmountReceived)
            .to_scaled_rational(token_in.decimals);
        let amount_out = U256::from(logs.makerAmountReceived)
            .to_scaled_rational(token_out.decimals);

        Ok(NormalizedSwap {
            protocol: Protocol::Dexalot,
            trace_index: info.trace_idx,
            from: logs.taker,
            recipient: logs.taker,
            msg_value: info.msg_value,
            pool: info.target_address,
            token_in,
            token_out,
            amount_in,
            amount_out
        })
    }
);
#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{hex, Address, B256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        db::token_info::TokenInfoWithAddress, normalized_actions::Action, TreeSearchBuilder,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_dexalot_simple_swap() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let swap_tx =
            B256::from(hex!("60ecdc3ff51bcb3599fc4e1111a81d136f093237d293a45ce92c6318a1dfcad5"));

        let token_in = TokenInfoWithAddress::weth();
        let token_out = TokenInfoWithAddress::usdc();

        let amount_in = U256::from_str("6005863988421")
            .unwrap()
            .to_scaled_rational(token_in.decimals);
        let amount_out = U256::from_str("10000")
            .unwrap()
            .to_scaled_rational(token_out.decimals);

        let action = Action::Swap(NormalizedSwap {
            protocol: Protocol::Dexalot,
            trace_index: 0,
            from: Address::from_str("0x0000Ea7fbDeAdE231816CC098A4d270d8394066B").unwrap(),
            recipient: Address::from_str("0x0000Ea7fbDeAdE231816CC098A4d270d8394066B").unwrap(),
            msg_value: U256::ZERO,
            pool: Address::from_str("0xdef1c0ded9bec7f1a1670819833240f027b25eff").unwrap(),
            token_in,
            token_out,
            amount_in,
            amount_out,
        });

        classifier_utils
            .contains_action(
                swap_tx,
                0,
                action,
                TreeSearchBuilder::default().with_action(Action::is_swap),
            )
            .await
            .unwrap()
    }
    #[brontes_macros::test]
    async fn test_dexalot_partial_swap() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let swap_tx =
            B256::from(hex!("60ecdc3ff51bcb3599fc4e1111a81d136f093237d293a45ce92c6318a1dfcad5"));

        let token_in = TokenInfoWithAddress::weth();
        let token_out = TokenInfoWithAddress::usdc();

        let amount_in = U256::from_str("6005863988421")
            .unwrap()
            .to_scaled_rational(token_in.decimals);
        let amount_out = U256::from_str("10000")
            .unwrap()
            .to_scaled_rational(token_out.decimals);

        let action = Action::Swap(NormalizedSwap {
            protocol: Protocol::Dexalot,
            trace_index: 0,
            from: Address::from_str("0x0000Ea7fbDeAdE231816CC098A4d270d8394066B").unwrap(),
            recipient: Address::from_str("0x0000Ea7fbDeAdE231816CC098A4d270d8394066B").unwrap(),
            msg_value: U256::ZERO,
            pool: Address::from_str("0xdef1c0ded9bec7f1a1670819833240f027b25eff").unwrap(),
            token_in,
            token_out,
            amount_in,
            amount_out,
        });

        classifier_utils
            .contains_action(
                swap_tx,
                0,
                action,
                TreeSearchBuilder::default().with_action(Action::is_swap),
            )
            .await
            .unwrap()
    }
}
