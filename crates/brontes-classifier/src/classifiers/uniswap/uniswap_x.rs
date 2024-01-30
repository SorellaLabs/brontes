use alloy_primitives::Address;
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::normalized_actions::NormalizedBatch;

action_impl!(
    Protocol::UniswapX,
    crate::UniswapX::executeCall,
    Batch,
    [..Fill],
    call_data: true,
    logs: true,
    |trace_index,
    _from_address: Address,
    target_address: Address,
    _msg_sender: Address,
    _call_data: executeCall,
    logs_data: UniswapXexecuteCallLogs,
    _db_tx: &DB| {
        //TODO: When the fill is a vec, iterate over to get each user order
        //TODO: Could also manually resolve order & decode inputs

        let fill_event = logs_data.Fill_field;

        Some(NormalizedBatch {
            protocol: Protocol::UniswapX,
            trace_index,
            solver: fill_event.filler,
            settlement_contract: target_address,
            user_swaps: Vec::new(),
            solver_swaps: Some(Vec::new()),
        })
    }
);

action_impl!(
    Protocol::UniswapX,
    crate::UniswapX::executeBatchCall,
    Batch,
    [..Fill*],
    call_data: true,
    |trace_index,
    from_address: Address,
    target_address: Address,
    _msg_sender: Address,
    _call_data: executeBatchCall,
    _db_tx: &DB| {
        Some(NormalizedBatch {
            protocol: Protocol::UniswapX,
            trace_index,
            solver: from_address,
            settlement_contract: target_address,
            user_swaps: Vec::new(),
            solver_swaps: Some(Vec::new()),
        })
    }
);

action_impl!(
    Protocol::UniswapX,
    crate::UniswapX::executeBatchWithCallbackCall,
    Batch,
    [..Fill*],
    call_data: true,
    logs: true,
    |trace_index,
    from_address: Address,
    target_address: Address,
    _msg_sender: Address,
    _call_data: executeBatchWithCallbackCall,
    _log_data: UniswapXexecuteBatchWithCallbackCallLogs,
    _db_tx: &DB| {
        Some(NormalizedBatch {
            protocol: Protocol::UniswapX,
            trace_index,
            solver: from_address,
            settlement_contract: target_address,
            user_swaps: Vec::new(),
            solver_swaps: Some(Vec::new()),
        })
    }
);

action_impl!(
    Protocol::UniswapX,
    crate::UniswapX::executeWithCallbackCall,
    Batch,
    [..Fill],
    call_data: true,
    |trace_index,
    from_address: Address,
    target_address: Address,
    _msg_sender: Address,
    _call_data: executeWithCallbackCall,
    _db_tx: &DB| {
        Some(NormalizedBatch {
            protocol: Protocol::UniswapX,
            trace_index,
            solver: from_address,
            settlement_contract: target_address,
            user_swaps: Vec::new(),
            solver_swaps: Some(Vec::new()),
        })
    }
);
