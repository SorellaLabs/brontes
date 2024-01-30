use alloy_primitives::Address;
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::{NormalizedFlashLoan, NormalizedLiquidation},
    utils::ToScaledRational,
};
use malachite::{num::basic::traits::Zero, Rational};

action_impl!(
    Protocol::AaveV3,
    crate::AaveV3::liquidationCallCall,
    Liquidation,
    [LiquidationEvent],
    call_data: true,
    |trace_index,
    _from_address: Address,
    target_address: Address,
    msg_sender: Address,
    call_data: liquidationCallCall,
    db_tx: &DB | {

        let debt_info = db_tx.try_get_token_info(call_data.debtAsset).ok()??;
        let collateral_info = db_tx.try_get_token_info(call_data.collateralAsset).ok()??;

        let covered_debt = call_data.debtToCover.to_scaled_rational(debt_info.decimals);

        return Some(NormalizedLiquidation {
            protocol: Protocol::AaveV3,
            trace_index,
            pool: target_address,
            liquidator: msg_sender,
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
    |trace_index,
    from_address: Address,
    target_address: Address,
    _msg_sender: Address,
    call_data: flashLoanCall,
    db_tx: &DB | {
        let (amounts, assets): (Vec<_>, Vec<_>) = call_data.assets
            .iter()
            .zip(call_data.amounts.iter())
            .filter_map(|(asset, amount)| {
                let token_info = db_tx.try_get_token_info(*asset).ok()??;
                Some((amount.to_scaled_rational(token_info.decimals),token_info))
        }).unzip();

        return Some(NormalizedFlashLoan {
            protocol: Protocol::AaveV3,
            trace_index,
            from: from_address,
            pool: target_address,
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
    |trace_index,
    from_address: Address,
    target_address: Address,
    _msg_sender: Address,
    call_data: flashLoanSimpleCall,
    db_tx: &DB | {

        let token_info = db_tx.try_get_token_info(call_data.asset).ok()??;
        let amount = call_data.amount.to_scaled_rational(token_info.decimals);

        return Some(NormalizedFlashLoan {
            protocol: Protocol::AaveV3,
            trace_index,
            from: from_address,
            pool: target_address,
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
