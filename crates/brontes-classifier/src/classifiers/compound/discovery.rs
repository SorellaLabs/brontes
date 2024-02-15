use alloy_primitives::Address;
use brontes_macros::discovery_impl;
use brontes_pricing::Protocol;

discovery_impl!(
    CompoundV2Discovery,
    crate::CompoundV2Comptroller::_supportMarketCall,
    0x3d9819210A31b4961b30EF54bE2aeD79B9c9Cd3B,
    |deployed_address: Address, trace_index: u64, call_data: _supportMarketCall, _| async move {
        let token = call_data.cToken;

        vec![NormalizedNewPool {
            pool_address: deployed_address,
            trace_index,
            protocol: Protocol::CompoundV2,
            tokens: vec![token],
        }]
    }
);
