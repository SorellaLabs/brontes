use std::sync::Arc;

use alloy_primitives::{Address, U256};
use alloy_sol_types::SolCall;
use brontes_macros::discovery_impl;
use brontes_pricing::{make_call_request, types::DiscoveredPool};
use brontes_types::{traits::TracingProvider, Protocol};
use itertools::Itertools;

discovery_impl!(
    CurveV1MetapoolBaseDecoder,
    crate::CurveV1MetapoolFactory::add_base_poolCall,
    0x0959158b6040d32d04c301a72cbfd6b39e21c9ae,
    |deployed_address: Address, call: add_base_poolCall, tracer: Arc<T>| {
        parse_base_bool(Protocol::CurveV1BasePool, deployed_address, call._base_pool, tracer)
    }
);

discovery_impl!(
    CurveV1MetapoolMetaDecoder,
    crate::CurveV1MetapoolFactory::deploy_metapoolCall,
    0x0959158b6040d32d04c301a72cbfd6b39e21c9ae,
    |deployed_address: Address, call: deploy_metapoolCall, tracer: Arc<T>| {
        parse_meta_pool(
            Protocol::CurveV1MetaPool,
            deployed_address,
            call._base_pool,
            call._coin,
            tracer,
        )
    }
);

discovery_impl!(
    CurveV2MetapoolBaseDecoder,
    crate::CurveV2MetapoolFactory::add_base_poolCall,
    0xB9fC157394Af804a3578134A6585C0dc9cc990d4,
    |deployed_address: Address, call: add_base_poolCall, tracer: Arc<T>| {
        parse_base_bool(Protocol::CurveV2BasePool, call._base_pool, deployed_address, tracer)
    }
);

discovery_impl!(
    CurveV2MetapoolMetaDecoder0,
    crate::CurveV2MetapoolFactory::deploy_metapool_0Call,
    0xB9fC157394Af804a3578134A6585C0dc9cc990d4,
    |deployed_address: Address, call: deploy_metapool_0Call, tracer: Arc<T>| {
        parse_meta_pool(
            Protocol::CurveV2MetaPool,
            deployed_address,
            call._base_pool,
            call._coin,
            tracer,
        )
    }
);

discovery_impl!(
    CurveV2MetapoolMetaDecoder1,
    crate::CurveV2MetapoolFactory::deploy_metapool_1Call,
    0xB9fC157394Af804a3578134A6585C0dc9cc990d4,
    |deployed_address: Address, call: deploy_metapool_1Call, tracer: Arc<T>| {
        parse_meta_pool(
            Protocol::CurveV2MetaPool,
            deployed_address,
            call._base_pool,
            call._coin,
            tracer,
        )
    }
);

discovery_impl!(
    CurveV2MetapoolPlainDecoder0,
    crate::CurveV2MetapoolFactory::deploy_plain_pool_0Call,
    0xB9fC157394Af804a3578134A6585C0dc9cc990d4,
    |deployed_address: Address, call: deploy_plain_pool_0Call, _| {
        parse_plain_pool(Protocol::CurveV2PlainPool, deployed_address, call._coins)
    }
);

discovery_impl!(
    CurveV2MetapoolPlainDecoder1,
    crate::CurveV2MetapoolFactory::deploy_plain_pool_1Call,
    0xB9fC157394Af804a3578134A6585C0dc9cc990d4,
    |deployed_address: Address, call: deploy_plain_pool_1Call, _| {
        parse_plain_pool(Protocol::CurveV2PlainPool, deployed_address, call._coins)
    }
);
discovery_impl!(
    CurveV2MetapoolPlainDecoder2,
    crate::CurveV2MetapoolFactory::deploy_plain_pool_2Call,
    0xB9fC157394Af804a3578134A6585C0dc9cc990d4,
    |deployed_address: Address, call: deploy_plain_pool_2Call, _| {
        parse_plain_pool(Protocol::CurveV2PlainPool, deployed_address, call._coins)
    }
);

// discovery_impl!(
//     CurvecrvUSDBaseDecoder,
//     CurvecrvUSDFactory,
//     BasePoolAdded,
//     true,
//     false,
//     |protocol: Protocol,
//      decoded_events: Vec<(alloy_primitives::Log<crvUSDBasePoolAdded>, u64)>|
// {         curve_base_pool!(protocol, decoded_events)
//     }
// );
//
// discovery_impl!(
//     CurvecrvUSDMetaDecoder,
//     CurvecrvUSDFactory,
//     MetaPoolDeployed,
//     false,
//     true,
//     |node_handle: Arc<T>,
//      protocol: Protocol,
//      decoded_events: Vec<(
//         alloy_primitives::Log<crvUSDMetaPoolDeployed>,
//         u64,
//         Pin<Box<dyn Future<Output = Option<Address>> + Send>>
//     )>| { curve_meta_pool!(protocol, decoded_events) }
// );
//
// discovery_impl!(
//     CurvecrvUSDPlainDecoder,
//     CurvecrvUSDFactory,
//     PlainPoolDeployed,
//     false,
//     true,
//     |node_handle: Arc<T>,
//      protocol: Protocol,
//      decoded_events: Vec<(
//         alloy_primitives::Log<crvUSDPlainPoolDeployed>,
//         u64,
//         Pin<Box<dyn Future<Output = Option<Address>> + Send>>
//     )>| { curve_plain_pool!(protocol, decoded_events) }
// );

alloy_sol_types::sol!(
    function coins(uint256 arg0) external view returns (address);
);

async fn query_base_pool<T: TracingProvider>(tracer: &Arc<T>, base_pool: Address) -> Vec<Address> {
    let mut result = Vec::new();
    let mut i = 0;
    loop {
        let Ok(call) =
            make_call_request(coinsCall::new((U256::from(i),)), tracer, base_pool, None).await
        else {
            break
        };

        i += 1;
        result.push(call._0);
    }
    result
}

async fn parse_plain_pool<const N: usize>(
    protocol: Protocol,
    deployed_address: Address,
    tokens: [Address; N],
) -> Vec<DiscoveredPool> {
    tokens
        .into_iter()
        .permutations(2)
        .map(|tokens| DiscoveredPool::new(tokens, deployed_address, protocol))
        .collect_vec()
}

async fn parse_meta_pool<T: TracingProvider>(
    protocol: Protocol,
    deployed_address: Address,
    base_pool: Address,
    meta_token: Address,
    tracer: Arc<T>,
) -> Vec<DiscoveredPool> {
    let mut base_tokens = query_base_pool(&tracer, base_pool).await;
    base_tokens.push(meta_token);
    base_tokens
        .into_iter()
        .permutations(2)
        .map(|tokens| DiscoveredPool::new(tokens, deployed_address, protocol))
        .collect_vec()
}

async fn parse_base_bool<T: TracingProvider>(
    protocol: Protocol,
    deployed_address: Address,
    base_pool: Address,
    tracer: Arc<T>,
) -> Vec<DiscoveredPool> {
    query_base_pool(&tracer, base_pool)
        .await
        .into_iter()
        .permutations(2)
        .map(|tokens| DiscoveredPool::new(tokens, deployed_address, protocol))
        .collect_vec()
}
