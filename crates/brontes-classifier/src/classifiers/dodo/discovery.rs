use alloy_primitives::Address;
use brontes_macros::discovery_impl;
use brontes_pricing::Protocol;

// rustfmt::skip
discovery_impl!(
    DodoDVMDiscovery,
    crate::DodoDVMFactory::createDODOVendingMachineCall,
    0x72d220cE168C4f361dD4deE5D826a01AD8598f6C,
    |deployed_address: Address, trace_index: u64, call_data: createDODOVendingMachineCall, _| async move {
        let base_token = call_data.baseToken;
        let quote_token = call_data.quoteToken;

        vec![NormalizedNewPool {
            pool_address: deployed_address,
            trace_index,
            protocol: Protocol::Dodo,
            tokens: vec![base_token, quote_token],
        }]
    }
);

// rustfmt::skip
discovery_impl!(
    DodoDSPDiscovery,
    crate::DodoDSPFactory::createDODOStablePoolCall,
    0x6fdDB76c93299D985f4d3FC7ac468F9A168577A4,
    |deployed_address: Address, trace_index: u64, call_data: createDODOStablePoolCall, _| async move {
        let base_token = call_data.baseToken;
        let quote_token = call_data.quoteToken;

        vec![NormalizedNewPool {
            pool_address: deployed_address,
            trace_index,
            protocol: Protocol::Dodo,
            tokens: vec![base_token, quote_token],
        }]
    }
);
