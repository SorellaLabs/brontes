use alloy_primitives::Address;
use brontes_macros::discovery_impl;
use brontes_pricing::Protocol;

discovery_impl!(
    PancakeSwapV3Decoder,
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
// Add v2 : 0xcA143Ce32Fe78f1f7019d7d551a6402fC5350c73
