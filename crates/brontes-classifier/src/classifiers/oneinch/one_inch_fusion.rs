use alloy_primitives::Address;
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{normalized_actions::NormalizedAggregator, structured_trace::CallInfo};

action_impl!(
    Protocol::OneInchFusion,
    crate::OneInchFusionSettlement::settleOrdersCall,
    Aggregator,
    [],
    |info: CallInfo, _db_tx: &DB| {
        return Ok(NormalizedAggregator {
            protocol: Protocol::OneInchFusion,
            trace_index: info.trace_idx,
            from: info.from_address,
            to: info.target_address,
            recipient: Address::default(),
            child_actions: vec![],
            msg_value: info.msg_value,
        });
    }
);

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{hex, B256, U256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        db::token_info::TokenInfoWithAddress,
        normalized_actions::{Action, NormalizedSwap, NormalizedTransfer},
        Protocol::{ClipperExchange, OneInchFusion},
        ToScaledRational, TreeSearchBuilder,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_one_inch_fusion_swap() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let aggregator =
            B256::from(hex!("83860dfeec88e76c46cbfc945e6b3e80d2a355495f78567bdd91ee01e6220946"));

        let eq_action = Action::Aggregator(NormalizedAggregator {
            protocol: OneInchFusion,
            trace_index: 0,
            to: Address::new(hex!("a88800cd213da5ae406ce248380802bd53b47647")),
            from: Address::new(hex!("D14699b6B02e900A5C2338700d5181a674FDB9a2")),
            recipient: Address::new(hex!("d10F17699137DD6215c01F539726227fC042c9b2")),
            child_actions: vec![
                Action::Transfer(NormalizedTransfer {
                    trace_index: 5,
                    msg_value: U256::ZERO,
                    from: Address::new(hex!("d10f17699137dd6215c01f539726227fc042c9b2")),
                    to: Address::new(hex!("235d3afac42f5e5ff346cb6c19af13194988551f")),
                    token: TokenInfoWithAddress::usdc(),
                    amount: U256::from_str("269875186").unwrap().to_scaled_rational(6),
                    fee: U256::from_str("0").unwrap().to_scaled_rational(1),
                }),
                Action::Transfer(NormalizedTransfer {
                    trace_index: 9,
                    msg_value: U256::ZERO,
                    from: Address::new(hex!("235d3afac42f5e5ff346cb6c19af13194988551f")),
                    to: Address::new(hex!("655edce464cc797526600a462a8154650eee4b77")),
                    token: TokenInfoWithAddress::usdc(),
                    amount: U256::from_str("269875186").unwrap().to_scaled_rational(6),
                    fee: U256::from_str("0").unwrap().to_scaled_rational(1),
                }),
                Action::Swap(NormalizedSwap {
                    protocol: ClipperExchange,
                    trace_index: 11,
                    from: Address::new(hex!("235d3afac42f5e5ff346cb6c19af13194988551f")),
                    recipient: Address::new(hex!("235d3afac42f5e5ff346cb6c19af13194988551f")),
                    pool: Address::new(hex!("655edce464cc797526600a462a8154650eee4b77")),
                    token_in: TokenInfoWithAddress::usdc(),
                    token_out: TokenInfoWithAddress::usdt(),
                    amount_in: U256::from_str("269875186").unwrap().to_scaled_rational(6),
                    amount_out: U256::from_str("269716012").unwrap().to_scaled_rational(6),
                    msg_value: U256::ZERO,
                }),
                Action::Transfer(NormalizedTransfer {
                    trace_index: 15,
                    from: Address::new(hex!("655edce464cc797526600a462a8154650eee4b77")),
                    to: Address::new(hex!("235d3afac42f5e5ff346cb6c19af13194988551f")),
                    token: TokenInfoWithAddress::usdt(),
                    amount: U256::from_str("269716012").unwrap().to_scaled_rational(6),
                    fee: U256::from_str("0").unwrap().to_scaled_rational(1),
                    msg_value: U256::ZERO,
                }),
                Action::Transfer(NormalizedTransfer {
                    trace_index: 16,
                    from: Address::new(hex!("235d3afac42f5e5ff346cb6c19af13194988551f")),
                    to: Address::new(hex!("a88800cd213da5ae406ce248380802bd53b47647")),
                    token: TokenInfoWithAddress::usdt(),
                    amount: U256::from_str("216122672").unwrap().to_scaled_rational(6),
                    fee: U256::from_str("0").unwrap().to_scaled_rational(1),
                    msg_value: U256::ZERO,
                }),
                Action::Transfer(NormalizedTransfer {
                    trace_index: 18,
                    from: Address::new(hex!("a88800cd213da5ae406ce248380802bd53b47647")),
                    to: Address::new(hex!("d10f17699137dd6215c01f539726227fc042c9b2")),
                    token: TokenInfoWithAddress::usdt(),
                    amount: U256::from_str("216122672").unwrap().to_scaled_rational(6),
                    fee: U256::from_str("0").unwrap().to_scaled_rational(1),
                    msg_value: U256::ZERO,
                }),
            ],

            msg_value: U256::ZERO,
        });

        classifier_utils
            .contains_action(
                aggregator,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Action::is_aggregator),
            )
            .await
            .unwrap();
    }
}
