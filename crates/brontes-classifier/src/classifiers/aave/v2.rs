use brontes_macros::action_impl;
use brontes_types::{
    normalized_actions::{NormalizedFlashLoan, NormalizedLiquidation},
    structured_trace::CallInfo,
    utils::ToScaledRational,
    Protocol,
};
use malachite::{num::basic::traits::Zero, Rational};

action_impl!(
    Protocol::AaveV2,
    crate::AaveV2Pool::liquidationCallCall,
    Liquidation,
    [],
    call_data: true,
    |
    info: CallInfo,
    call_data: liquidationCallCall,
    db_tx: &DB| {
        let debt_info = db_tx.try_fetch_token_info(call_data.debtAsset)?;
        let collateral_info = db_tx.try_fetch_token_info(call_data.collateralAsset)?;

        let covered_debt = call_data.debtToCover.to_scaled_rational(debt_info.decimals);

        return Ok(NormalizedLiquidation {
            protocol: Protocol::AaveV2,
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
    Protocol::AaveV2,
    crate::AaveV2Pool::flashLoanCall,
    FlashLoan,
    [],
    call_data: true,
    |
    info: CallInfo,
    call_data: flashLoanCall,
    db_tx: &DB| {
        let (amounts, assets): (Vec<_>, Vec<_>) = call_data.assets
            .iter()
            .zip(call_data.amounts.iter())
            .filter_map(|(asset, amount)| {
                let token_info = db_tx.try_fetch_token_info(*asset).ok()?;
                Some((amount.to_scaled_rational(token_info.decimals),token_info))
        }).unzip();

        return Ok(NormalizedFlashLoan {
            protocol: Protocol::AaveV2,
            trace_index: info.trace_idx,
            from: info.msg_sender,
            pool: info.target_address,
            receiver_contract: call_data.receiverAddress,
            assets ,
            amounts,
            aave_mode: Some((call_data.modes, call_data.onBehalfOf)),
            // Set to zero at this stage, will be calculated upon finalized classification
            child_actions: vec![],
            repayments: vec![],
            fees_paid: vec![],
            msg_value: info.msg_value,
        })

    }
);
