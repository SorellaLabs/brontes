use alloy_primitives::Address;
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::normalized_actions::NormalizedBatch;

action_impl!(
    Protocol::UniswapX,
    crate::UniswapX::executeCall,
    Batch,
    [Fill],
    call_data: true,
    |trace_index,
    from_address: Address,
    target_address: Address,
    _msg_sender: Address,
    _call_data: executeCall,
    _db_tx: &DB| {
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
