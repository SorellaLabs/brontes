use alloy_primitives::U256;
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::NormalizedBatch,
    structured_trace::CallInfo,
};


action_impl!(
    Protocol::Cowswap,
    crate::CowswapGPv2Settlement::swapCall,
    Batch,
    [Trade*],
    call_data: true,
    logs: true,
    |
    info: CallInfo,
    _call_data: swapCall,
    _log_data: CowswapswapCallLogs,
    _db_tx: &DB| {
        Ok(NormalizedBatch{ 
            protocol: Protocol::Cowswap, 
            trace_index: info.trace_idx, 
            solver: info.msg_sender, 
            settlement_contract: info.target_address, 
            user_swaps: todo!(), 
            solver_swaps: todo!(), 
            msg_value: info.msg_value
        })
    }
);
