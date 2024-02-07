use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{normalized_actions::NormalizedBatch, structured_trace::CallInfo};

action_impl!(
    Protocol::UniswapX,
    crate::UniswapX::executeCall,
    Batch,
    [..Fill],
    call_data: true,
    logs: true,
    |
    info: CallInfo,
    _call_data: executeCall,
    logs_data: UniswapXexecuteCallLogs,
    _db_tx: &DB| {
        //TODO: When the fill is a vec, iterate over to get each user order
        //TODO: Could also manually resolve order & decode inputs

        let fill_event = logs_data.Fill_field;

        Ok(NormalizedBatch {
            protocol: Protocol::UniswapX,
            trace_index: info.trace_idx,
            solver: fill_event.filler,
            settlement_contract: info.target_address,
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
    |
    info: CallInfo,
    _call_data: executeBatchCall,
    _db_tx: &DB| {
        Ok(NormalizedBatch {
            protocol: Protocol::UniswapX,
            trace_index: info.trace_idx,
            solver: info.from_address,
            settlement_contract: info.target_address,
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
    |
    info: CallInfo,
    _call_data: executeBatchWithCallbackCall,
    _log_data: UniswapXexecuteBatchWithCallbackCallLogs,
    _db_tx: &DB| {
        Ok(NormalizedBatch {
            protocol: Protocol::UniswapX,
            trace_index: info.trace_idx,
            solver: info.from_address,
            settlement_contract: info.target_address,
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
    |
    info: CallInfo,
    _call_data: executeWithCallbackCall,
    _db_tx: &DB| {
        Ok(NormalizedBatch {
            protocol: Protocol::UniswapX,
            trace_index: info.trace_idx,
            solver: info.from_address,
            settlement_contract: info.target_address,
            user_swaps: Vec::new(),
            solver_swaps: Some(Vec::new()),
        })
    }
);
