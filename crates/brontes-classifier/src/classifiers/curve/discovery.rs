use std::{pin::Pin, sync::Arc};

use alloy_primitives::{Address, Log, B256};
use alloy_rpc_types::Filter;
use alloy_sol_types::{SolCall, SolEvent};
use brontes_macros::{discovery_dispatch, discovery_impl};
use brontes_types::{exchanges::StaticBindingsDb, traits::TracingProvider};
use futures::{future::join_all, Future};
use itertools::Itertools;

use crate::
    CURVE_BASE_POOLS_TOKENS,
;

macro_rules! curve_plain_pool {
    ($protocol:expr, $deployed_address:expr, $tokens:expr) => {
        $tokens.into_iter().permutations(2).map(|tokens| {
                ::brontes_pricing::types::DiscoveredPool::new(tokens, $deployed_address, $protocol)
        }).collect::<Vec<_>>()
    };
}

macro_rules! curve_meta_pool {
    ($protocol:expr, $deployed_address:expr, $base_pool:expr, $meta_token:expr) => {
        let mut base_tokens = CURVE_BASE_POOLS_TOKENS
            .get(&$base_pool_addr)
            .unwrap()
            .clone();
        base_tokens.push($meta_token);

        base_tokens
            .into_iter()
            .permutations(2)
            .map(|tokens| {
                ::brontes_pricing::types::DiscoveredPool::new(tokens, $deployed_address, $protocol)
            })
            .collect::<Vec<_>>()
    };
}

macro_rules! curve_base_pool {
    ($protocol:expr, $deployed_address, $base_pool_address:expr) => {
        CURVE_BASE_POOLS_TOKENS
            .get(&$base_pool_addr)
            .unwrap()
            .clone()
            .into_iter()
            .permutations(2)
            .map(|tokens| {
                ::brontes_pricing::types::DiscoveredPool::new(tokens, $deployed_address, $protocol)
            })
            .collect::<Vec<_>>()
    };
}

discovery_impl!(
    CurveV1MetapoolBaseDecoder,
    crate::CurveV1MetapoolFactory::add_base_poolCall,
    0x0959158b6040d32d04c301a72cbfd6b39e21c9ae,
    |deployed_address: Address, call: add_base_poolCall| {
        curve_base_pool!(StaticBindingsDb::CurveV1BasePool, deployed_address, call._base_pool)
    }
);

discovery_impl!(
    CurveV1MetapoolMetaDecoder,
    crate::CurveV1MetapoolFactory::deploy_metapoolCall,
    0x0959158b6040d32d04c301a72cbfd6b39e21c9ae,
    |deployed_address: Address, call: deploy_metapoolCall| {
         curve_meta_pool!(StaticBindingsDb::CurveV1MetaPool, deployed_address, call._base_pool, call._coin)
    }
);


discovery_impl!(
    CurveV2MetapoolBaseDecoder,
    crate::CurveV2MetapoolFactory::add_base_poolCall,
    0xB9fC157394Af804a3578134A6585C0dc9cc990d4,
    |deployed_address: Address, _| {
        curve_base_pool!(StaticBindingsDb::CurveV2BasePool, deployed_address)
    }
);

discovery_impl!(
    CurveV2MetapoolMetaDecoder,
    crate::CurveV2MetapoolFactory::deploy_metapool_0Call,
    0xB9fC157394Af804a3578134A6585C0dc9cc990d4,
    |deployed_address: Address, call: deploy_metapool_0Call | {
        curve_meta_pool!(StaticBindingsDb::CurveV2MetaPool, deployed_address, call._base_pool, call._coin)
    }
);

discovery_impl!(
    CurveV2MetapoolMetaDecoder,
    crate::CurveV2MetapoolFactory::deploy_metapool_1Call,
    0xB9fC157394Af804a3578134A6585C0dc9cc990d4,
    |deployed_address: Address, call: deploy_metapool_1Call | {
        curve_meta_pool!(StaticBindingsDb::CurveV2MetaPool, deployed_address, call._base_pool, call._coin)
    }
);

discovery_impl!(
    CurveV2MetapoolPlainDecoder,
    crate::CurveV2MetapoolFactory::deploy_plain_pool_0Call,
    0xB9fC157394Af804a3578134A6585C0dc9cc990d4,
    |deployed_address: Address, call: deploy_plain_pool_0Call| {
        curve_plain_pool!(StaticBindingsDb::CurveV2PlainPool, deployed_address, call._coins) 
    }
);

discovery_impl!(
    CurveV2MetapoolPlainDecoder,
    crate::CurveV2MetapoolFactory::deploy_plain_pool_1Call,
    0xB9fC157394Af804a3578134A6585C0dc9cc990d4,
    |deployed_address: Address, call: deploy_plain_pool_1Call| {
        curve_plain_pool!(StaticBindingsDb::CurveV2PlainPool, deployed_address, call._coins) 
    }
);
discovery_impl!(
    CurveV2MetapoolPlainDecoder,
    crate::CurveV2MetapoolFactory::deploy_plain_pool_2Call,
    0xB9fC157394Af804a3578134A6585C0dc9cc990d4,
    |deployed_address: Address, call: deploy_plain_pool_2Call| {
        curve_plain_pool!(StaticBindingsDb::CurveV2PlainPool, deployed_address, call._coins) 
    }
);

discovery_impl!(
    CurvecrvUSDBaseDecoder,
    CurvecrvUSDFactory,
    BasePoolAdded,
    true,
    false,
    |protocol: StaticBindingsDb,
     decoded_events: Vec<(alloy_primitives::Log<crvUSDBasePoolAdded>, u64)>| {
        curve_base_pool!(protocol, decoded_events)
    }
);

discovery_impl!(
    CurvecrvUSDMetaDecoder,
    CurvecrvUSDFactory,
    MetaPoolDeployed,
    false,
    true,
    |node_handle: Arc<T>,
     protocol: StaticBindingsDb,
     decoded_events: Vec<(
        alloy_primitives::Log<crvUSDMetaPoolDeployed>,
        u64,
        Pin<Box<dyn Future<Output = Option<Address>> + Send>>
    )>| { curve_meta_pool!(protocol, decoded_events) }
);

discovery_impl!(
    CurvecrvUSDPlainDecoder,
    CurvecrvUSDFactory,
    PlainPoolDeployed,
    false,
    true,
    |node_handle: Arc<T>,
     protocol: StaticBindingsDb,
     decoded_events: Vec<(
        alloy_primitives::Log<crvUSDPlainPoolDeployed>,
        u64,
        Pin<Box<dyn Future<Output = Option<Address>> + Send>>
    )>| { curve_plain_pool!(protocol, decoded_events) }
);
