use std::sync::Arc;

use alloy_primitives::Log;
use alloy_sol_types::SolEvent;
use brontes_macros::{discovery_dispatch, discovery_impl};
use brontes_types::{exchanges::StaticBindingsDb, traits::TracingProvider};

use super::{DiscoveredPool, FactoryDecoder, FactoryDecoderDispatch};
use crate::{
    UniswapV2Factory, UniswapV2Factory::PairCreated, UniswapV3Factory,
    UniswapV3Factory::PoolCreated,
};

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
