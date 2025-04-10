use brontes_macros::action_impl;
use brontes_types::{
    normalized_actions::{NormalizedFlashLoan, NormalizedLiquidation},
    structured_trace::CallInfo,
    utils::ToScaledRational,
    Protocol,
};
use malachite::{num::basic::traits::Zero, Rational};

use crate::aave_v3_bindings::AaveV3Pool;

action_impl!(
    Protocol::AaveV3Pool,
    AaveV3Pool::liquidationCallCall,
    Liquidation,
    [LiquidationEvent],
    call_data: true,
    |
    info: CallInfo,
    call_data: liquidationCallCall,
    db_tx: &DB | {

        let debt_info = db_tx.try_fetch_token_info(call_data.debtAsset)?;
        let collateral_info = db_tx.try_fetch_token_info(call_data.collateralAsset)?;

        let covered_debt = call_data.debtToCover.to_scaled_rational(debt_info.decimals);

        return Ok(NormalizedLiquidation {
            protocol: Protocol::AaveV3Pool,
            trace_index: info.trace_idx,
            pool: info.from_address,
            liquidator: info.msg_sender,
            debtor: call_data.user,
            collateral_asset: collateral_info,
            debt_asset: debt_info,
            covered_debt,
            // filled in later
            liquidated_collateral: Rational::ZERO,
            msg_value: info.msg_value,
        })
    }
);

action_impl!(
    Protocol::AaveV3Pool,
    AaveV3Pool::flashLoanCall,
    FlashLoan,
    [],
    call_data: true,
    |
    info: CallInfo,
    call_data: flashLoanCall,
    db_tx: &DB | {
        let (amounts, assets): (Vec<_>, Vec<_>) = call_data.assets
            .iter()
            .zip(call_data.amounts.iter())
            .filter_map(|(asset, amount)| {
                let token_info = db_tx.try_fetch_token_info(*asset).ok()?;
                Some((amount.to_scaled_rational(token_info.decimals),token_info))
        }).unzip();

        return Ok(NormalizedFlashLoan {
            protocol: Protocol::AaveV3Pool,
            trace_index: info.trace_idx,
            from: info.from_address,
            pool: info.target_address,
            receiver_contract: call_data.receiverAddress,
            assets,
            amounts,
            aave_mode: Some((call_data.interestRateModes, call_data.onBehalfOf)),
            // These fields are all empty at this stage, they will be filled upon finalized classification
            child_actions: vec![],
            repayments: vec![],
            fees_paid: vec![],
            msg_value: info.msg_value,


        })

    }
);

action_impl!(
    Protocol::AaveV3Pool,
    AaveV3Pool::flashLoanSimpleCall,
    FlashLoan,
    [],
    call_data: true,
    |
    info: CallInfo,
    call_data: flashLoanSimpleCall,
    db_tx: &DB | {

        let token_info = db_tx.try_fetch_token_info(call_data.asset)?;
        let amount = call_data.amount.to_scaled_rational(token_info.decimals);

        return Ok(NormalizedFlashLoan {
            protocol: Protocol::AaveV3Pool,
            trace_index: info.trace_idx,
            from: info.from_address,
            pool: info.target_address,
            receiver_contract: call_data.receiverAddress,
            assets: vec![token_info],
            amounts: vec![amount],
            aave_mode: None,
            // These fields are all empty at this stage, they will be filled upon finalized classification
            child_actions: vec![],
            repayments: vec![],
            fees_paid: vec![],
            msg_value: info.msg_value,


        })

    }
);

#[cfg(test)]
mod tests {
    use alloy_primitives::{hex, Address, B256, U256};
    use brontes_types::{
        normalized_actions::{Action, NormalizedLiquidation},
        Protocol, TreeSearchBuilder,
    };
    use malachite::Rational;

    use crate::test_utils::ClassifierTestUtils;

    #[brontes_macros::test]
    async fn test_aave_v3_liquidation() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let aave_v3_liquidation =
            B256::from(hex!("dd951e0fc5dc4c98b8daaccdb750ff3dc9ad24a7f689aad2a088757266ab1d55"));

        let eq_action = Action::Liquidation(NormalizedLiquidation {
            protocol:              Protocol::AaveV3Pool,
            liquidated_collateral: Rational::from_signeds(165516722, 100000000),
            covered_debt:          Rational::from_signeds(63857746423_i64, 1000000),
            debtor:                Address::from(hex!("e967954b9b48cb1a0079d76466e82c4d52a8f5d3")),
            debt_asset:            classifier_utils
                .get_token_info(Address::from(hex!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"))),
            collateral_asset:      classifier_utils
                .get_token_info(Address::from(hex!("2260fac5e5542a773aa44fbcfedf7c193bc2c599"))),
            liquidator:            Address::from(hex!("80d4230c0a68fc59cb264329d3a717fcaa472a13")),
            pool:                  Address::from(hex!("87870bca3f3fd6335c3f4ce8392d69350b4fa4e2")),
            trace_index:           6,
            msg_value:             U256::ZERO,
        });

        classifier_utils
            .contains_action(
                aave_v3_liquidation,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Action::is_liquidation),
            )
            .await
            .unwrap();
    }
}
