mod base2;
pub use base2::*;

mod base3;
pub use base3::*;

mod base4;
pub use base4::*;

pub mod lido2;
pub use lido2::CurveBasePool2Remove_liquidity_one_coinCall as CurveBasePool2LidoRemove_liquidity_one_coinCall;

#[cfg(test)]
mod tests {

    use alloy_primitives::{hex, Address, B256, U256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        db::token_info::{TokenInfo, TokenInfoWithAddress},
        normalized_actions::{Action, NormalizedBurn},
        Protocol, ToScaledRational, TreeSearchBuilder,
    };

    #[brontes_macros::test]
    async fn test_curve_base_remove_liquidity() {
        let classifier_utils = ClassifierTestUtils::new().await;
        classifier_utils.ensure_protocol(
            Protocol::CurveBasePool3,
            Address::new(hex!("7fC77b5c7614E1533320Ea6DDc2Eb61fa00A9714")),
            Address::new(hex!("EB4C2781e4ebA804CE9a9803C67d0893436bB27D")),
            Some(Address::new(hex!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599"))),
            Some(Address::new(hex!("fE18be6b3Bd88A2D2A7f928d00292E7a9963CfC6"))),
            None,
            None,
            None,
        );

        let burn =
            B256::from(hex!("9de52c88215f3252d27c7778f265b52600fd49e0a8c31b48047299dbba0cabf0"));

        let token0 = TokenInfoWithAddress {
            address: Address::new(hex!("EB4C2781e4ebA804CE9a9803C67d0893436bB27D")),
            inner:   TokenInfo { decimals: 8, symbol: "renBTC".to_string() },
        };

        let token1 = TokenInfoWithAddress {
            address: Address::new(hex!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599")),
            inner:   TokenInfo { decimals: 8, symbol: "WBTC".to_string() },
        };

        let token2 = TokenInfoWithAddress {
            address: Address::new(hex!("fE18be6b3Bd88A2D2A7f928d00292E7a9963CfC6")),
            inner:   TokenInfo { decimals: 18, symbol: "sBTC".to_string() },
        };

        classifier_utils.ensure_token(token0.clone());
        classifier_utils.ensure_token(token1.clone());
        classifier_utils.ensure_token(token2.clone());

        let eq_action = Action::Burn(NormalizedBurn {
            protocol:    Protocol::CurveBasePool3,
            trace_index: 0,
            from:        Address::new(hex!("aEBd1F6272Bc7E2d406595cc2E98AAE21a47F03d")),
            recipient:   Address::new(hex!("aEBd1F6272Bc7E2d406595cc2E98AAE21a47F03d")),
            pool:        Address::new(hex!("7fC77b5c7614E1533320Ea6DDc2Eb61fa00A9714")),
            token:       vec![token0, token1, token2],
            amount:      vec![
                U256::from(135971).to_scaled_rational(8),
                U256::from(253273).to_scaled_rational(8),
                U256::from(2022770990903219_u128).to_scaled_rational(18),
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

    #[brontes_macros::test]
    async fn test_curve_base_remove_liquidity_imbalanced() {
        let classifier_utils = ClassifierTestUtils::new().await;
        classifier_utils.ensure_protocol(
            Protocol::CurveBasePool3,
            Address::new(hex!("7fC77b5c7614E1533320Ea6DDc2Eb61fa00A9714")),
            Address::new(hex!("EB4C2781e4ebA804CE9a9803C67d0893436bB27D")),
            Some(Address::new(hex!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599"))),
            Some(Address::new(hex!("fE18be6b3Bd88A2D2A7f928d00292E7a9963CfC6"))),
            None,
            None,
            None,
        );

        let burn =
            B256::from(hex!("3f17151032cb3e3ae039b140e465c3cf3f9ff8cb593109817dd0526eb0300150"));

        let token0 = TokenInfoWithAddress {
            address: Address::new(hex!("EB4C2781e4ebA804CE9a9803C67d0893436bB27D")),
            inner:   TokenInfo { decimals: 8, symbol: "renBTC".to_string() },
        };

        let token1 = TokenInfoWithAddress {
            address: Address::new(hex!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599")),
            inner:   TokenInfo { decimals: 8, symbol: "WBTC".to_string() },
        };

        let token2 = TokenInfoWithAddress {
            address: Address::new(hex!("fE18be6b3Bd88A2D2A7f928d00292E7a9963CfC6")),
            inner:   TokenInfo { decimals: 18, symbol: "sBTC".to_string() },
        };

        classifier_utils.ensure_token(token0.clone());
        classifier_utils.ensure_token(token1.clone());

        let eq_action = Action::Burn(NormalizedBurn {
            protocol:    Protocol::CurveBasePool3,
            trace_index: 0,
            from:        Address::new(hex!("13ca2cf84365BD2daffd4A7e364Ea11388607C37")),
            recipient:   Address::new(hex!("13ca2cf84365BD2daffd4A7e364Ea11388607C37")),
            pool:        Address::new(hex!("7fC77b5c7614E1533320Ea6DDc2Eb61fa00A9714")),
            token:       vec![token0, token1, token2],
            amount:      vec![
                U256::from(0).to_scaled_rational(8),
                U256::from(50000000).to_scaled_rational(8),
                U256::from(0).to_scaled_rational(18),
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

    #[brontes_macros::test]
    async fn test_curve_base_remove_liquidity_one() {
        let classifier_utils = ClassifierTestUtils::new().await;
        classifier_utils.ensure_protocol(
            Protocol::CurveBasePool3,
            Address::new(hex!("7fC77b5c7614E1533320Ea6DDc2Eb61fa00A9714")),
            Address::new(hex!("EB4C2781e4ebA804CE9a9803C67d0893436bB27D")),
            Some(Address::new(hex!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599"))),
            Some(Address::new(hex!("fE18be6b3Bd88A2D2A7f928d00292E7a9963CfC6"))),
            None,
            None,
            None,
        );

        let burn =
            B256::from(hex!("054098af5b21c4e95a46b88a2a7d093b83bfdee448a732d3396925f48f4225c3"));

        let token = TokenInfoWithAddress {
            address: Address::new(hex!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599")),
            inner:   TokenInfo { decimals: 8, symbol: "WBTC".to_string() },
        };

        classifier_utils.ensure_token(token.clone());

        let eq_action = Action::Burn(NormalizedBurn {
            protocol:    Protocol::CurveBasePool3,
            trace_index: 0,
            from:        Address::new(hex!("045929aF66312685d143B96C9d44Ce5ddCBAB768")),
            recipient:   Address::new(hex!("045929aF66312685d143B96C9d44Ce5ddCBAB768")),
            pool:        Address::new(hex!("7fC77b5c7614E1533320Ea6DDc2Eb61fa00A9714")),
            token:       vec![token],
            amount:      vec![U256::from(38855798316741927_u128).to_scaled_rational(8)],
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
