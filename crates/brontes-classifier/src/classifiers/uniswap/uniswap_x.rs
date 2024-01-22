use alloy_primitives::Address;
use brontes_database::libmdbx::tx::CompressedLibmdbxTx;
use brontes_macros::action_impl;
use brontes_types::normalized_actions::NormalizedBatch;
use reth_db::mdbx::RO;

use crate::UniswapX::executeCall;

action_impl!(
    Protocol::UniswapX,
    Batch,
    executeCall,
    [Fill],
    UniswapX,
    call_data: true,
    logs: true,
    |trace_index,
    from_address: Address,
    target_address: Address,
    msg_sender: Address,
    call_data: executeCall,
    logs: UniXExecuteImplBatch,
    db_tx: &CompressedLibmdbxTx<RO>| {
        let logs = logs.Fill_field;

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
