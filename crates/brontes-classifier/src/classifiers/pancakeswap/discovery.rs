use alloy_primitives::Address;
use brontes_macros::discovery_impl;
use brontes_pricing::Protocol;

discovery_impl!(
    PancakeSwapV2Discovery,
    crate::UniswapV2Factory::createPairCall,
    0x1097053Fd2ea711dad45caCcc45EfF7548fCB362,
    |deployed_address: Address, trace_index: u64, call_data: createPairCall, _| async move {
        let token_a = call_data.tokenA;
        let token_b = call_data.tokenB;

        vec![NormalizedNewPool {
            pool_address: deployed_address,
            trace_index,
            protocol: Protocol::SushiSwapV2,
            tokens: vec![token_a, token_b],
        }]
    }
);

discovery_impl!(
    PancakeSwapV3Discovery,
    crate::UniswapV3Factory::createPoolCall,
    0x0BFbCF9fa4f9C56B0F40a671Ad40E0805A091865,
    |deployed_address: Address, trace_index: u64, call_data: createPoolCall, _| async move {
        let token_a = call_data.tokenA;
        let token_b = call_data.tokenB;

        vec![NormalizedNewPool {
            pool_address: deployed_address,
            trace_index,
            protocol: Protocol::PancakeSwapV3,
            tokens: vec![token_a, token_b],
        }]
    }
);
