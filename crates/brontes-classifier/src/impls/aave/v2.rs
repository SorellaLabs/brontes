use alloy_primitives::{Address, Bytes};
use alloy_sol_types::SolCall;
use brontes_database_libmdbx::{implementation::tx::LibmdbxTx, tables::AddressToTokens};
use brontes_macros::{action_dispatch, action_impl};
use brontes_pricing::types::PoolUpdate;
use brontes_types::normalized_actions::{Actions, NormalizedLiquidation};
use reth_db::{mdbx::RO, transaction::DbTx};
use reth_rpc_types::Log;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    enum_unwrap,
    AaveV2::{liquidationCallCall, AaveV2Calls},
    ActionCollection, IntoAction, StaticReturnBindings,
};

action_impl!(
    LiquidationCallImplV2,
    Liquidation,
    liquidationCallCall,
    NormalizedLiquidation,
    AaveV2,
    call_data: true,
    |index, from_address: Address, target_address: Address, call_data: liquidationCallCall, db_tx: &LibmdbxTx<RO> | {

        let tokens = db_tx.get::<AddressToTokens>(target_address).ok()??;
        let [mut token_0, mut token_1] = [tokens.token0, tokens.token1];


        return Some(NormalizedLiquidation {
            trace_index: index,
            pool: target_address,
            liquidator: from_address,
            debtor: call_data.user,
            collateral_asset: call_data.collateralAsset,
            debt_asset: call_data.debtAsset,
            amount: call_data.debtToCover,
        })
    }
);

action_dispatch!(AaveV2Classifier, LiquidationCallImplV2);
