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
        db::token_info::TokenInfoWithAddress, normalized_actions::Actions, Protocol::OneInchFusion,
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
            child_actions: vec![],

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
            B256::from(hex!("883f9df921c8e01926e8ef975b886899b75de98bab2659eca61d12590cbd87c5"));

        let eq_action = Actions::Aggregator(NormalizedAggregator {
            protocol:      OneInchFusion,
            trace_index:   0,
            from:          Address::new(hex!("D14699b6B02e900A5C2338700d5181a674FDB9a2")),
            recipient:     Address::new(hex!("96E3e323966713a1f56dbb5D5bFabB28B2e4B428")),
            pool:          Address::new(hex!("A88800CD213dA5Ae406ce248380802BD53b47647")),
            token_in:      TokenInfoWithAddress::usdc(),
            amount_in:     U256::from_str("1234614915").unwrap().to_scaled_rational(6),
            token_out:     TokenInfoWithAddress::weth(),
            amount_out:    U256::from_str("354748864757954269")
                .unwrap()
                .to_scaled_rational(18),
            child_actions: vec![],

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
