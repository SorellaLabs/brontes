use alloy_primitives::B256;
use brontes_macros::{discovery_dispatch, discovery_impl};
use brontes_pricing::types::DiscoveredPool;
use brontes_types::exchanges::StaticBindingsDb;

use crate::{UniswapV2Factory::PairCreated, UniswapV3Factory::PoolCreated};

discovery_impl!(
    UniswapV2Decoder,
    UniswapV2Factory,
    PairCreated,
    true,
    false,
    |protocol: StaticBindingsDb, decoded_events: Vec<(alloy_primitives::Log<PairCreated>, u64)>| {
        decoded_events
            .into_iter()
            .map(|(evt, block_number)| {
                DiscoveredPool::new(vec![evt.token0, evt.token1], evt.pair, protocol)
            })
            .collect::<Vec<_>>()
    }
);

discovery_impl!(
    UniswapV3Decoder,
    UniswapV3Factory,
    PoolCreated,
    true,
    false,
    |protocol: StaticBindingsDb, decoded_events: Vec<(alloy_primitives::Log<PoolCreated>, u64)>| {
        decoded_events
            .into_iter()
            .map(|(evt, block_number)| {
                DiscoveredPool::new(vec![evt.token0, evt.token1], evt.pool, protocol)
            })
            .collect::<Vec<_>>()
    }
);

discovery_dispatch!(UniswapDecoder, UniswapV2Decoder, UniswapV3Decoder);
