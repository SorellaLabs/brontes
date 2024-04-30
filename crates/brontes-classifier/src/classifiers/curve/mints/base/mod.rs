mod base2;
pub use base2::*;

mod base3;
pub use base3::*;

mod base4;
pub use base4::*;

#[cfg(test)]
mod tests {

    use alloy_primitives::{hex, Address, B256, U256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        db::token_info::{TokenInfo, TokenInfoWithAddress},
        normalized_actions::{Action, NormalizedMint},
        Protocol, ToScaledRational, TreeSearchBuilder,
    };

    #[brontes_macros::test]
    async fn test_curve_base_add_liquidity() {
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

        let mint =
            B256::from(hex!("dbf57244aad3402faa04e1ff19d3af0f89e1ac9aff3dd3830d2d6415b4dfdc0c"));

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

        let eq_action = Action::Mint(NormalizedMint {
            protocol:    Protocol::CurveBasePool3,
            trace_index: 0,
            from:        Address::new(hex!("DaD7ef2EfA3732892d33aAaF9B3B1844395D9cbE")),
            recipient:   Address::new(hex!("DaD7ef2EfA3732892d33aAaF9B3B1844395D9cbE")),
            pool:        Address::new(hex!("7fC77b5c7614E1533320Ea6DDc2Eb61fa00A9714")),
            token:       vec![token0, token1, token2],
            amount:      vec![
                U256::from(0).to_scaled_rational(8),
                U256::from(27506).to_scaled_rational(8),
                U256::from(0).to_scaled_rational(18),
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
}
