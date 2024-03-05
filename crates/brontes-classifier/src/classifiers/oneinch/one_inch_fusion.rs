use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    db::token_info::TokenInfoWithAddress, normalized_actions::NormalizedAggregator,
    structured_trace::CallInfo,
};
use malachite::{num::basic::traits::Zero, Rational};
use reth_primitives::Address;

action_impl!(
    Protocol::OneInchFusion,
    crate::OneInchFusionSettlement::settleOrdersCall,
    Aggregator,
    [],
    |info: CallInfo, _db_tx: &DB| {
        return Ok(NormalizedAggregator {
            protocol:      Protocol::OneInchFusion,
            trace_index:   info.trace_idx,
            from:          info.from_address,
            recipient:     Address::default(),
            pool:          info.target_address,
            token_in:      TokenInfoWithAddress::default(),
            token_out:     TokenInfoWithAddress::default(),
            amount_in:     Rational::ZERO,
            amount_out:    Rational::ZERO,
            child_actions: vec![],
            msg_value:     info.msg_value,
        })
    }
);

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{hex, Address, B256, U256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        db::token_info::TokenInfoWithAddress,
        normalized_actions::{Actions, NormalizedSwap, NormalizedTransfer},
        Protocol::{ClipperExchange, OneInchFusion},
        ToScaledRational, TreeSearchBuilder,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_one_inch_fusion_swap() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let aggregator =
            B256::from(hex!("83860dfeec88e76c46cbfc945e6b3e80d2a355495f78567bdd91ee01e6220946"));

        let eq_action = Actions::Aggregator(NormalizedAggregator {
            protocol:      OneInchFusion,
            trace_index:   0,
            from:          Address::new(hex!("D14699b6B02e900A5C2338700d5181a674FDB9a2")),
            recipient:     Address::new(hex!("d10F17699137DD6215c01F539726227fC042c9b2")),
            pool:          Address::new(hex!("A88800CD213dA5Ae406ce248380802BD53b47647")),
            token_in:      TokenInfoWithAddress::usdc(),
            amount_in:     U256::from_str("269875186").unwrap().to_scaled_rational(6),
            token_out:     TokenInfoWithAddress::usdt(),
            amount_out:    U256::from_str("216122672").unwrap().to_scaled_rational(6),
            child_actions: vec![
                Actions::Swap(NormalizedSwap {
                    protocol:    ClipperExchange,
                    trace_index: 11,
                    from:        Address::new(hex!("235d3afac42f5e5ff346cb6c19af13194988551f")),
                    recipient:   Address::new(hex!("235d3afac42f5e5ff346cb6c19af13194988551f")),
                    pool:        Address::new(hex!("655edce464cc797526600a462a8154650eee4b77")),
                    token_in:    TokenInfoWithAddress::usdc(),
                    token_out:   TokenInfoWithAddress::usdt(),
                    amount_in:   U256::from_str("269875186").unwrap().to_scaled_rational(6),
                    amount_out:  U256::from_str("269716012").unwrap().to_scaled_rational(6),
                    msg_value:   U256::ZERO,
                }),
                Actions::Transfer(NormalizedTransfer {
                    trace_index: 5,
                    from:        Address::new(hex!("d10f17699137dd6215c01f539726227fc042c9b2")),
                    to:          Address::new(hex!("235d3afac42f5e5ff346cb6c19af13194988551f")),
                    token:       TokenInfoWithAddress::usdc(),
                    amount:      U256::from_str("269875186").unwrap().to_scaled_rational(6),
                    fee:         U256::from_str("0").unwrap().to_scaled_rational(1),
                }),
                Actions::Transfer(NormalizedTransfer {
                    trace_index: 9,
                    from:        Address::new(hex!("235d3afac42f5e5ff346cb6c19af13194988551f")),
                    to:          Address::new(hex!("655edce464cc797526600a462a8154650eee4b77")),
                    token:       TokenInfoWithAddress::usdc(),
                    amount:      U256::from_str("269875186").unwrap().to_scaled_rational(6),
                    fee:         U256::from_str("0").unwrap().to_scaled_rational(1),
                }),
                Actions::Transfer(NormalizedTransfer {
                    trace_index: 15,
                    from:        Address::new(hex!("655edce464cc797526600a462a8154650eee4b77")),
                    to:          Address::new(hex!("235d3afac42f5e5ff346cb6c19af13194988551f")),
                    token:       TokenInfoWithAddress::usdt(),
                    amount:      U256::from_str("269716012").unwrap().to_scaled_rational(6),
                    fee:         U256::from_str("0").unwrap().to_scaled_rational(1),
                }),
                Actions::Transfer(NormalizedTransfer {
                    trace_index: 16,
                    from:        Address::new(hex!("235d3afac42f5e5ff346cb6c19af13194988551f")),
                    to:          Address::new(hex!("a88800cd213da5ae406ce248380802bd53b47647")),
                    token:       TokenInfoWithAddress::usdt(),
                    amount:      U256::from_str("216122672").unwrap().to_scaled_rational(6),
                    fee:         U256::from_str("0").unwrap().to_scaled_rational(1),
                }),
                Actions::Transfer(NormalizedTransfer {
                    trace_index: 18,
                    from:        Address::new(hex!("a88800cd213da5ae406ce248380802bd53b47647")),
                    to:          Address::new(hex!("d10f17699137dd6215c01f539726227fc042c9b2")),
                    token:       TokenInfoWithAddress::usdt(),
                    amount:      U256::from_str("216122672").unwrap().to_scaled_rational(6),
                    fee:         U256::from_str("0").unwrap().to_scaled_rational(1),
                }),
            ],

            msg_value: U256::ZERO,
        });

        classifier_utils
            .contains_action(
                aggregator,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Actions::is_aggregator),
            )
            .await
            .unwrap();
    }

    #[brontes_macros::test]
    async fn test_one_inch_fusion_swap_weth() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let aggregator =
            B256::from(hex!("41814ea8244e7783db7f847f68d5b10b93bf410bb14afddaa208bd5bb9ddecfe"));

        let eq_action = Actions::Aggregator(NormalizedAggregator {
            protocol:      OneInchFusion,
            trace_index:   0,
            from:          Address::new(hex!("D14699b6B02e900A5C2338700d5181a674FDB9a2")),
            recipient:     Address::new(hex!("c2DEfea119d3E2916783BDB6e346eC804230Ed7B")),
            pool:          Address::new(hex!("A88800CD213dA5Ae406ce248380802BD53b47647")),
            token_in:      TokenInfoWithAddress::usdc(),
            amount_in:     U256::from_str("165882572").unwrap().to_scaled_rational(6),
            token_out:     TokenInfoWithAddress::weth(),
            amount_out:    U256::from_str("34402796677143920")
                .unwrap()
                .to_scaled_rational(18),
            child_actions: vec![
                Actions::Swap(NormalizedSwap {
                    protocol:    ClipperExchange,
                    trace_index: 15,
                    from:        Address::new(hex!("235d3afac42f5e5ff346cb6c19af13194988551f")),
                    recipient:   Address::new(hex!("235d3afac42f5e5ff346cb6c19af13194988551f")),
                    pool:        Address::new(hex!("655edce464cc797526600a462a8154650eee4b77")),
                    token_in:    TokenInfoWithAddress::usdc(),
                    token_out:   TokenInfoWithAddress::weth(),
                    amount_in:   U256::from_str("165882572").unwrap().to_scaled_rational(6),
                    amount_out:  U256::from_str("47652261091733496")
                        .unwrap()
                        .to_scaled_rational(18),
                    msg_value:   U256::ZERO,
                }),
                Actions::Transfer(NormalizedTransfer {
                    trace_index: 9,
                    from:        Address::new(hex!("c2defea119d3e2916783bdb6e346ec804230ed7b")),
                    to:          Address::new(hex!("235d3afac42f5e5ff346cb6c19af13194988551f")),
                    token:       TokenInfoWithAddress::usdc(),
                    amount:      U256::from_str("165882572").unwrap().to_scaled_rational(6),
                    fee:         U256::from_str("0").unwrap().to_scaled_rational(1),
                }),
                Actions::Transfer(NormalizedTransfer {
                    trace_index: 13,
                    from:        Address::new(hex!("235d3afac42f5e5ff346cb6c19af13194988551f")),
                    to:          Address::new(hex!("655edce464cc797526600a462a8154650eee4b77")),
                    token:       TokenInfoWithAddress::usdc(),
                    amount:      U256::from_str("165882572").unwrap().to_scaled_rational(6),
                    fee:         U256::from_str("0").unwrap().to_scaled_rational(1),
                }),
                Actions::Transfer(NormalizedTransfer {
                    trace_index: 19,
                    from:        Address::new(hex!("655edce464cc797526600a462a8154650eee4b77")),
                    to:          Address::new(hex!("235d3afac42f5e5ff346cb6c19af13194988551f")),
                    token:       TokenInfoWithAddress::weth(),
                    amount:      U256::from_str("47652261091733496")
                        .unwrap()
                        .to_scaled_rational(18),
                    fee:         U256::from_str("0").unwrap().to_scaled_rational(1),
                }),
                Actions::Transfer(NormalizedTransfer {
                    trace_index: 20,
                    from:        Address::new(hex!("235d3afac42f5e5ff346cb6c19af13194988551f")),
                    to:          Address::new(hex!("a88800cd213da5ae406ce248380802bd53b47647")),
                    token:       TokenInfoWithAddress::weth(),
                    amount:      U256::from_str("3440279667714392")
                        .unwrap()
                        .to_scaled_rational(18),
                    fee:         U256::from_str("0").unwrap().to_scaled_rational(1),
                }),
                Actions::Transfer(NormalizedTransfer {
                    trace_index: 22,
                    from:        Address::new(hex!("a88800cd213da5ae406ce248380802bd53b47647")),
                    to:          Address::new(hex!("08b067ad41e45babe5bbb52fc2fe7f692f628b06")),
                    token:       TokenInfoWithAddress::weth(),
                    amount:      U256::from_str("3440279667714392")
                        .unwrap()
                        .to_scaled_rational(18),
                    fee:         U256::from_str("0").unwrap().to_scaled_rational(1),
                }),
            ],

            msg_value: U256::ZERO,
        });

        classifier_utils
            .contains_action(
                aggregator,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Actions::is_aggregator),
            )
            .await
            .unwrap();
    }
}
