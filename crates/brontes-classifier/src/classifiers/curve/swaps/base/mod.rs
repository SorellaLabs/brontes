mod base2;
pub use base2::*;

mod base3;
pub use base3::*;

mod base4;
pub use base4::*;

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{hex, Address, B256, U256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        db::token_info::{TokenInfo, TokenInfoWithAddress},
        normalized_actions::{Action, NormalizedSwap},
        Protocol, ToScaledRational, TreeSearchBuilder,
    };

    #[brontes_macros::test]
    async fn test_curve_base_exchange() {
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

        let swap =
            B256::from(hex!("6987133dd8ee7f5f76615a7484418905933625305a948350b38e924a905c0ef6"));

        let token_in = TokenInfoWithAddress {
            address: Address::new(hex!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599")),
            inner:   TokenInfo { decimals: 8, symbol: "WBTC".to_string() },
        };

        let token_out = TokenInfoWithAddress {
            address: Address::new(hex!("EB4C2781e4ebA804CE9a9803C67d0893436bB27D")),
            inner:   TokenInfo { decimals: 8, symbol: "renBTC".to_string() },
        };

        classifier_utils.ensure_token(token_in.clone());
        classifier_utils.ensure_token(token_out.clone());

        let eq_action = Action::Swap(NormalizedSwap {
            protocol: Protocol::CurveBasePool3,
            trace_index: 0,
            from: Address::new(hex!("0F5cd3C453A7FCD7735eB2f0493F36D41398A4a0")),
            recipient: Address::new(hex!("0F5cd3C453A7FCD7735eB2f0493F36D41398A4a0")),
            pool: Address::new(hex!("7fC77b5c7614E1533320Ea6DDc2Eb61fa00A9714")),
            token_in,
            amount_in: U256::from_str("61733447").unwrap().to_scaled_rational(8),
            token_out,
            amount_out: U256::from_str("61329579").unwrap().to_scaled_rational(8),
            msg_value: U256::ZERO,
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
