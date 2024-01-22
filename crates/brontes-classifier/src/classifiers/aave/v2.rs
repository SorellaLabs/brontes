use alloy_primitives::{Address, U256};
use brontes_database::libmdbx::{tables::AddressToTokens, tx::CompressedLibmdbxTx};
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::normalized_actions::{NormalizedFlashLoan, NormalizedLiquidation};
use reth_db::mdbx::RO;

use crate::AaveV2::{flashLoanCall, liquidationCallCall};

action_impl!(
    Protocol::AaveV2,
    Liquidation,
    liquidationCallCall,
    [],
    AaveV2,
    call_data: true,
    |trace_index,
    from_address: Address,
    target_address: Address,
    msg_sender: Address,
    call_data: liquidationCallCall,
    db_tx: &CompressedLibmdbxTx<RO>| {
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
    FlashLoan,
    flashLoanCall,
    [],
    AaveV2,
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
