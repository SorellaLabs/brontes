use alloy_primitives::{Address, U256};
use brontes_database::libmdbx::{tables::AddressToTokens, tx::CompressedLibmdbxTx};
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::normalized_actions::{NormalizedFlashLoan, NormalizedLiquidation};
use reth_db::mdbx::RO;

action_impl!(
    Protocol::AaveV2,
    crate::AaveV2::liquidationCallCall,
    Liquidation,
    [],
    call_data: true,
    |trace_index,
    from_address: Address,
    target_address: Address,
    msg_sender: Address,
    call_data: liquidationCallCall,
    _db_tx: &DB| {
        return Some(NormalizedLiquidation {
            trace_index,
            pool: target_address,
            liquidator: msg_sender,
            debtor: call_data.user,
            collateral_asset: call_data.collateralAsset,
            debt_asset: call_data.debtAsset,
            covered_debt: call_data.debtToCover,
            // filled in later
            liquidated_collateral: U256::ZERO,
        })
    }
);

action_impl!(
    Protocol::AaveV2,
    crate::AaveV2::flashLoanCall,
    FlashLoan,
    [],
    call_data: true,
    |trace_index,
    from_address: Address,
    target_address: Address,
    msg_sender: Address,
    call_data: flashLoanCall,
    _db_tx: &DB| {
        return Some(NormalizedFlashLoan {
            trace_index,
            from: msg_sender,
            pool: target_address,
            receiver_contract: call_data.receiverAddress,
            assets: call_data.assets,
            amounts: call_data.amounts,
            aave_mode: Some((call_data.modes, call_data.onBehalfOf)),
            // Set to zero at this stage, will be calculated upon finalized classification
            child_actions: vec![],
            repayments: vec![],
            fees_paid: vec![],
        })

    }
);
