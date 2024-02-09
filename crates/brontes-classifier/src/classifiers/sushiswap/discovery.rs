use alloy_primitives::Address;
use brontes_macros::discovery_impl;
use brontes_pricing::{types::DiscoveredPool, Protocol};

discovery_impl!(
    SushiSwapV2Decoder,
    crate::UniswapV2Factory::createPairCall,
    0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac,
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
    SushiSwapV3Decoder,
    crate::UniswapV3Factory::createPoolCall,
    0xbACEB8eC6b9355Dfc0269C18bac9d6E2Bdc29C4F,
    |deployed_address: Address, trace_index: u64, call_data: createPoolCall, _| async move {
        let token_a = call_data.tokenA;
        let token_b = call_data.tokenB;

        vec![NormalizedNewPool {
            pool_address: deployed_address,
            trace_index,
            protocol: Protocol::SushiSwapV3,
            tokens: vec![token_a, token_b],
        }]
    }
);
