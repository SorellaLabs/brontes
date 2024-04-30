use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::NormalizedLiquidation, structured_trace::CallInfo, utils::ToScaledRational,
};

action_impl!(
    Protocol::CompoundV2,
    crate::CompoundV2CToken::liquidateBorrowCall,
    Liquidation,
    [..LiquidateBorrow],
    call_data: true,
    logs: true,
    |
    info: CallInfo,
    call_data: liquidateBorrowCall,
    log_data: CompoundV2LiquidateBorrowCallLogs,
    db_tx: &DB | {
        let logs = log_data.liquidate_borrow_field?;
        let debt_asset = info.target_address;
        let debt_info = db_tx.try_fetch_token_info(debt_asset)?;
        let collateral = db_tx.try_fetch_token_info(call_data.cTokenCollateral)?;
        let debt_covered = logs.repayAmount.to_scaled_rational(debt_info.decimals);
        let collateral_liquidated = logs.seizeTokens.to_scaled_rational(collateral.decimals);
        return Ok(NormalizedLiquidation {
            protocol: Protocol::CompoundV2,
            trace_index: info.trace_idx,
            pool: info.target_address,
            liquidator: logs.liquidator,
            debtor: call_data.borrower,
            collateral_asset: collateral,
            debt_asset: debt_info,
            covered_debt: debt_covered,
            liquidated_collateral: collateral_liquidated,
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
    async fn test_compound_v2_liquidation() {
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
