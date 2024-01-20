use alloy_primitives::{Address, U256};
use brontes_database_libmdbx::{tables::AddressToTokens, tx::CompressedLibmdbxTx};
use brontes_macros::{action_dispatch, action_impl};
use brontes_types::normalized_actions::{NormalizedFlashLoan, NormalizedLiquidation};
use reth_db::{mdbx::RO, transaction::DbTx};

use crate::AaveV3::{flashLoanCall, flashLoanSimpleCall, liquidationCallCall};

action_impl!(
    LiquidationCallImplV3,
    Liquidation,
    liquidationCallCall,
    [LiquidationEvent],
    AaveV3,
    call_data: true,
    |trace_index,
    from_address: Address,
    target_address: Address,
    msg_sender: Address,
    call_data: liquidationCallCall,
    db_tx: &CompressedLibmdbxTx<RO> | {
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
    FlashloanImplV3,
    FlashLoan,
    flashLoanCall,
    [],
    AaveV3,
    call_data: true,
    |trace_index,
    from_address: Address,
    target_address: Address,
    msg_sender: Address,
    call_data: flashLoanCall,
    db_tx: &CompressedLibmdbxTx<RO> | {

        let tokens = db_tx.get::<AddressToTokens>(target_address).ok()??;
        let [mut token_0, mut token_1] = [tokens.token0, tokens.token1];

        return Some(NormalizedFlashLoan {
            trace_index,
            from: from_address,
            pool: target_address,
            receiver_contract: call_data.receiverAddress,
            assets: call_data.assets,
            amounts: call_data.amounts,
            aave_mode: Some((call_data.interestRateModes, call_data.onBehalfOf)),
            // These fields are all empty at this stage, they will be filled upon finalized classification
            child_actions: vec![],
            repayments: vec![],
            fees_paid: vec![],


        })

    }
);

action_impl!(
    FlashloanSimpleImplV3,
    FlashLoan,
    flashLoanSimpleCall,
    [],
    AaveV3,
    call_data: true,
    |trace_index,
    from_address: Address,
    target_address: Address,
    msg_sender: Address,
    call_data: flashLoanSimpleCall,
    db_tx: &CompressedLibmdbxTx<RO> | {

        let tokens = db_tx.get::<AddressToTokens>(target_address).ok()??;
        let [mut token_0, mut token_1] = [tokens.token0, tokens.token1];

        return Some(NormalizedFlashLoan {
            trace_index,
            from: from_address,
            pool: target_address,
            receiver_contract: call_data.receiverAddress,
            assets: vec![call_data.asset],
            amounts: vec![call_data.amount],
            aave_mode: None,
            // These fields are all empty at this stage, they will be filled upon finalized classification
            child_actions: vec![],
            repayments: vec![],
            fees_paid: vec![],


        })

    }
);

action_dispatch!(AaveV3Classifier, LiquidationCallImplV3, FlashloanImplV3, FlashloanSimpleImplV3);
