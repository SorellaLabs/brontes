use alloy_primitives::{Address, Bytes, U256};
use alloy_sol_types::{SolCall, SolEvent};
use brontes_database_libmdbx::{implementation::tx::LibmdbxTx, tables::AddressToTokens};
use brontes_macros::{action_dispatch, action_impl};
use brontes_pricing::types::PoolUpdate;
use brontes_types::normalized_actions::{Actions, NormalizedAction, NormalizedBatch};
use reth_db::{mdbx::RO, transaction::DbTx};
use alloy_primitives::LogData;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    enum_unwrap, ActionCollection, IntoAction, StaticReturnBindings,
    UniswapX::{
        executeBatchCall, executeBatchWithCallbackCall, executeCall, executeWithCallbackCall, Fill,
        UniswapXCalls,
    },
};

action_impl!(
    UniXExecuteImpl,
    Batch,
    executeCall,
    [Fill],
    UniswapX,
    call_data: true,
    logs: true,
    |trace_index, from_address: Address, target_address: Address, call_data: executeCall, logs: Fill, db_tx: &LibmdbxTx<RO>| {

        //TODO: Finish implementing this
        Some(NormalizedBatch {
            trace_index,
            solver: from_address,
            settlement_contract: target_address,
            user_swaps: Vec::new(),
            solver_swaps: Some(Vec::new()),

        })
    }
);

action_dispatch!(UniswapXClassifier, UniXExecuteImpl);
