use std::{pin::Pin, sync::Arc};

use alloy_primitives::{Address, Log, B256};
use alloy_rpc_types::Filter;
use alloy_sol_types::SolEvent;
use brontes_macros::{discovery_dispatch, discovery_impl};
use brontes_types::{exchanges::StaticBindingsDb, traits::TracingProvider};
use futures::{future::join_all, Future};

use crate::{
    CurveV1MetapoolFactory::{
        self, BasePoolAdded as V1BasePoolAdded, MetaPoolDeployed as V1MetaPoolDeployed,
    },
    CurveV2MetapoolFactory::{
        self, BasePoolAdded as V2BasePoolAdded, MetaPoolDeployed as V2MetaPoolDeployed,
        PlainPoolDeployed as V2PlainPoolDeployed,
    },
    CurvecrvUSDFactory::{
        self, BasePoolAdded as crvUSDBasePoolAdded, MetaPoolDeployed as crvUSDMetaPoolDeployed,
        PlainPoolDeployed as crvUSDPlainPoolDeployed,
    },
    DiscoveredPool, CURVE_BASE_POOLS_TOKENS,
};

macro_rules! curve_plain_pool {
    ($protocol:expr, $decoded_events:expr) => {
        async move {
            join_all(
                $decoded_events
                    .into_iter()
                    .map(|(evt, block_number, pool_addr_future)| {
                        let protocol_clone = $protocol.clone();
                        async move {
                            let Some(pool_addr) = pool_addr_future.await else { return None };

                            Some(::brontes_pricing::types::DiscoveredPool::new(
                                evt.coins.to_vec(),
                                pool_addr,
                                protocol_clone,
                            ))
                        }
                    }),
            )
            .await
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
        }
    };
}

macro_rules! curve_meta_pool {
    ($protocol:expr, $decoded_events:expr) => {
        async move {
            join_all(
                $decoded_events
                    .into_iter()
                    .map(|(evt, block_number, pool_addr_future)| {
                        let protocol_clone = $protocol.clone();
                        async move {
                            let base_pool_addr: reth_primitives::Address =
                                evt.base_pool.clone().0 .0.into();
                            let mut pool_tokens = CURVE_BASE_POOLS_TOKENS
                                .get(&base_pool_addr)
                                .unwrap()
                                .clone();
                            pool_tokens.push(evt.coin.0 .0.into());
                            pool_tokens.sort();

                            let Some(pool_addr) = pool_addr_future.await else { return None };

                            Some(::brontes_pricing::types::DiscoveredPool::new(
                                pool_tokens
                                    .into_iter()
                                    .map(|token| token.0 .0.into())
                                    .collect(),
                                pool_addr,
                                protocol_clone,
                            ))
                        }
                    }),
            )
            .await
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
        }
    };
}

macro_rules! curve_base_pool {
    ($protocol:expr, $decoded_events:expr) => {
        $decoded_events
            .into_iter()
            .map(|(evt, block_number)| {
                let base_pool_addr: reth_primitives::Address = evt.base_pool.clone().0 .0.into();

                let mut pool_tokens = CURVE_BASE_POOLS_TOKENS
                    .get(&base_pool_addr)
                    .unwrap()
                    .clone();
                pool_tokens.sort();

                DiscoveredPool::new(
                    pool_tokens
                        .into_iter()
                        .map(|token| token.0 .0.into())
                        .collect(),
                    evt.base_pool.0 .0.into(),
                    $protocol.clone(),
                )
            })
            .collect::<Vec<_>>()
    };
}

discovery_impl!(
    CurveV1MetapoolBaseDecoder,
    CurveV1MetapoolFactory,
    BasePoolAdded,
    true,
    false,
    |protocol: StaticBindingsDb,
     decoded_events: Vec<(alloy_primitives::Log<V1BasePoolAdded>, u64)>| {
        curve_base_pool!(protocol, decoded_events)
    }
);

discovery_impl!(
    CurveV1MetapoolMetaDecoder,
    CurveV1MetapoolFactory,
    MetaPoolDeployed,
    false,
    true,
    |node_handle: Arc<T>,
     protocol: StaticBindingsDb,
     decoded_events: Vec<(
        alloy_primitives::Log<V1MetaPoolDeployed>,
        u64,
        Pin<Box<dyn Future<Output = Option<Address>> + Send>>
    )>| { curve_meta_pool!(protocol, decoded_events) }
);

discovery_impl!(
    CurveV2MetapoolBaseDecoder,
    CurveV2MetapoolFactory,
    BasePoolAdded,
    true,
    false,
    |protocol: StaticBindingsDb,
     decoded_events: Vec<(alloy_primitives::Log<V2BasePoolAdded>, u64)>| {
        curve_base_pool!(protocol, decoded_events)
    }
);

discovery_impl!(
    CurveV2MetapoolMetaDecoder,
    CurveV2MetapoolFactory,
    MetaPoolDeployed,
    false,
    true,
    |node_handle: Arc<T>,
     protocol: StaticBindingsDb,
     decoded_events: Vec<(
        alloy_primitives::Log<V2MetaPoolDeployed>,
        u64,
        Pin<Box<dyn Future<Output = Option<Address>> + Send>>
    )>| { curve_meta_pool!(protocol, decoded_events) }
);

discovery_impl!(
    CurveV2MetapoolPlainDecoder,
    CurveV2MetapoolFactory,
    PlainPoolDeployed,
    false,
    true,
    |node_handle: Arc<T>,
     protocol: StaticBindingsDb,
     decoded_events: Vec<(
        alloy_primitives::Log<V2PlainPoolDeployed>,
        u64,
        Pin<Box<dyn Future<Output = Option<Address>> + Send>>
    )>| { curve_plain_pool!(protocol, decoded_events) }
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

discovery_dispatch!(
    CurveDecoder,
    CurveV1MetapoolBaseDecoder,
    CurveV2MetapoolBaseDecoder,
    CurveV2MetapoolMetaDecoder,
    CurveV2MetapoolPlainDecoder,
    CurvecrvUSDBaseDecoder,
    CurvecrvUSDMetaDecoder,
    CurvecrvUSDPlainDecoder
);

pub async fn get_log_from_tx<T: TracingProvider>(
    node_handle: Arc<T>,
    block_num: u64,
    tx_hash: B256,
    log_topic_bench: B256,
    idicies_prior: usize,
) -> Option<Log> {
    let filter = Filter::new().from_block(block_num);

    let logs = match node_handle.logs_from_filter(filter).await {
        Ok(l) => l,
        Err(_) => return None,
    };

    let tx_logs = logs
        .into_iter()
        .filter(|log| log.transaction_hash.is_some())
        .filter(|log_tx| &log_tx.transaction_hash.unwrap() == &tx_hash)
        .collect::<Vec<_>>();

    let log_topic_bench: reth_primitives::TxHash = log_topic_bench.0.into();
    let Some((idx, _)) = tx_logs
        .iter()
        .enumerate()
        .find(|(_, log)| log.topics[0] == log_topic_bench)
    else {
        return None
    };

    Some(rpc_to_alloy_log(&tx_logs[idx - idicies_prior]))
}

fn rpc_to_alloy_log(log: &reth_rpc_types::Log) -> alloy_primitives::Log {
    alloy_primitives::Log::new_unchecked(
        log.address.0 .0.into(),
        log.topics
            .iter()
            .map(|topic| topic.0.into())
            .collect::<Vec<_>>(),
        log.data.0.clone().into(),
    )
}
