use alloy_primitives::Address;
use brontes_macros::action_impl;
use brontes_types::{
    normalized_actions::{NormalizedFlashLoan, NormalizedLiquidation},
    structured_trace::CallInfo,
    utils::ToScaledRational,
    Protocol,
};
use malachite::{num::basic::traits::Zero, Rational};

action_impl!(
    Protocol::AaveV3,
    crate::AaveV3::liquidationCallCall,
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
            protocol: Protocol::AaveV3,
            trace_index: info.trace_idx,
            pool: info.target_address,
            liquidator: info.msg_sender,
            debtor: call_data.user,
            collateral_asset: collateral_info,
            debt_asset: debt_info,
            covered_debt: covered_debt,
            // filled in later
            liquidated_collateral: Rational::ZERO,
        })
    }
);

action_impl!(
    Protocol::AaveV3,
    crate::AaveV3::flashLoanCall,
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
            protocol: Protocol::AaveV3,
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


        })

    }
);

action_impl!(
    Protocol::AaveV3,
    crate::AaveV3::flashLoanSimpleCall,
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
            protocol: Protocol::AaveV3,
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


        })

    }
);
