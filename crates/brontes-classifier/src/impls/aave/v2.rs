use alloy_primitives::{hex, Address, Bytes, FixedBytes};
use alloy_sol_types::{SolCall, SolEvent};
use brontes_database_libmdbx::{implementation::tx::LibmdbxTx, tables::AddressToTokens};
use brontes_macros::{action_dispatch, action_impl};
use brontes_pricing::types::PoolUpdate;
use brontes_types::normalized_actions::{Actions, NormalizedLiquidation};
use reth_db::{mdbx::RO, transaction::DbTx};
use reth_rpc_types::Log;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    enum_unwrap,
    AaveV2::{liquidationCallCall, AaveV2Calls, LiquidationCall as LiquidationCallEvent},
    ActionCollection, IntoAction, StaticReturnBindings,
};
pub const WETH: Address = Address(FixedBytes(hex!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")));

action_impl!(
    LiquidationCallImpl,
    Liquidation,
    liquidationCallCall,
    LiquidationCallEvent,
    AaveV2,
    logs: true,
    call_data: true,
    |index, from_address: Address, target_address: Address, call_data: liquidationCallCall, log_data: Option<LiquidationCallEvent>, db_tx: &LibmdbxTx<RO> | {
        let log = log_data?;

        let tokens = db_tx.get::<AddressToTokens>(target_address).ok()??;
        let [mut token_0, mut token_1] = [tokens.token0, tokens.token1];


        return Some(NormalizedLiquidation {
            index,
            pool: target_address,
            liquidator: from_address,
            debtor: call_data.user,
            collateral_asset: call_data.collateralAsset,
            debt_asset: call_data.debtAsset,
            amount: call_data.debtToCover,
            reward: todo!(),
        })
    }
);

action_dispatch!(AaveV2Classifier, LiquidationCallImpl);
