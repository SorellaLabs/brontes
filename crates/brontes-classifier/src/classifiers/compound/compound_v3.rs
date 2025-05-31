use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::NormalizedLiquidation, structured_trace::CallInfo, utils::ToScaledRational,
};
// TODO related to OEV(oracle based trading action is needed to implement)
action_impl!(
    Protocol::CompoundV3,
    crate::Comet::buyCollateralCall,
    Liquidation,
    [..BuyCollateral],
    logs: true,
    include_delegated_logs: true,
    |
    info: CallInfo,
    log_data: CompoundV3BuyCollateralCallLogs,
    db_tx: &DB | {
        let logs = log_data.buy_collateral_field?;
        let details=db_tx.get_protocol_details(info.target_address)?;

        let collateral_asset = logs.asset; // discounted collateral asset
        let collateral_info = db_tx.try_fetch_token_info(collateral_asset)?;
        let payment_asset=details.token0; // comet base token to pay
        let payment_info = db_tx.try_fetch_token_info(payment_asset)?;

        let payment_covered = logs.baseAmount.to_scaled_rational(payment_info.decimals);
        let collateral_sold = logs.collateralAmount.to_scaled_rational(collateral_info.decimals);
        return Ok(NormalizedLiquidation {
            protocol: Protocol::CompoundV3,
            trace_index: info.trace_idx,
            pool: info.target_address,
            liquidator: logs.buyer,
            debtor: info.target_address,
            collateral_asset: collateral_info,
            debt_asset: payment_info,
            covered_debt: payment_covered,
            liquidated_collateral: collateral_sold,
            msg_value: info.msg_value,
        })
    }
);

#[cfg(test)]
mod tests {
    use alloy_primitives::{hex, Address, B256, U256};
    use brontes_types::{
        db::token_info::TokenInfoWithAddress,
        normalized_actions::{Action, NormalizedLiquidation},
        Protocol, TreeSearchBuilder,
    };
    use malachite::Rational;

    use crate::test_utils::ClassifierTestUtils;

    #[brontes_macros::test]
    async fn test_compound_v3_auction_participation() {
        let classifier_utils = ClassifierTestUtils::new().await;
        classifier_utils.ensure_protocol(
            Protocol::CompoundV2,
            hex!("39aa39c021dfbae8fac545936693ac917d5e7563").into(),
            hex!("39aa39c021dfbae8fac545936693ac917d5e7563").into(),
            None,
            None,
            None,
            None,
            None,
        );

        let debt = TokenInfoWithAddress {
            address: hex!("39aa39c021dfbae8fac545936693ac917d5e7563").into(),
            inner:   brontes_types::db::token_info::TokenInfo {
                decimals: 8,
                symbol:   "cUSDC".to_string(),
            },
        };

        let collateral = TokenInfoWithAddress {
            address: hex!("70e36f6BF80a52b3B46b3aF8e106CC0ed743E8e4").into(),
            inner:   brontes_types::db::token_info::TokenInfo {
                decimals: 8,
                symbol:   "CompoundCollateral".to_string(),
            },
        };

        classifier_utils.ensure_token(debt);
        classifier_utils.ensure_token(collateral);

        let compound_v2_liquidation =
            B256::from(hex!("3a3ba6b0a6b69a8e316e1c20f97b9ce2de790b2f3bf90aaef5b29b06aafa5fda"));

        let eq_action = Action::Liquidation(NormalizedLiquidation {
            protocol:              Protocol::CompoundV2,
            liquidated_collateral: Rational::from_signeds(6140057900131i64, 100000000),
            covered_debt:          Rational::from_signeds(48779241727i64, 100000000),
            debtor:                Address::from(hex!("De74395831F3Ba9EdC7cBEE1fcB441cf24c0AF4d")),
            debt_asset:            classifier_utils
                .get_token_info(Address::from(hex!("39aa39c021dfbae8fac545936693ac917d5e7563"))),
            collateral_asset:      classifier_utils
                .get_token_info(Address::from(hex!("70e36f6BF80a52b3B46b3aF8e106CC0ed743E8e4"))),
            liquidator:            Address::from(hex!("D911560979B78821D7b045C79E36E9CbfC2F6C6F")),
            pool:                  Address::from(hex!("39AA39c021dfbaE8faC545936693aC917d5E7563")),
            trace_index:           2,
            msg_value:             U256::ZERO,
        });

        classifier_utils
            .contains_action(
                compound_v2_liquidation,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Action::is_liquidation),
            )
            .await
            .unwrap();
    }
}
