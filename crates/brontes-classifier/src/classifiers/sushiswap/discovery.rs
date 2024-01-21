use alloy_primitives::Address;
use brontes_macros::discovery_impl;
use brontes_pricing::types::DiscoveredPool;
use brontes_types::exchanges::StaticBindingsDb;

discovery_impl!(
    SushiSwapV2Decoder,
    crate::UniswapV2Factory::createPairCall,
    0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac,
    |deployed_address: Address, call_data: createPairCall| {
        let token_a = call_data.tokenA;
        let token_b = call_data.tokenB;

        vec![DiscoveredPool::new(
            vec![token_a, token_b],
            deployed_address,
            StaticBindingsDb::SushiSwapV2,
        )]
    }
);

discovery_impl!(
    SushiSwapV3Decoder,
    crate::UniswapV3Factory::createPoolCall,
    0xbACEB8eC6b9355Dfc0269C18bac9d6E2Bdc29C4F,
    |deployed_address: Address, call_data: createPoolCall| {
        let token_a = call_data.tokenA;
        let token_b = call_data.tokenB;

        vec![DiscoveredPool::new(
            vec![token_a, token_b],
            deployed_address,
            StaticBindingsDb::SushiSwapV3,
        )]
    }
);
