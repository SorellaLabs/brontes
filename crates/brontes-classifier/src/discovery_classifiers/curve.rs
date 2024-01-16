
use std::pin::Pin;

use alloy_primitives::Address;
use alloy_sol_types::SolEvent;
use eth_raw_pools_macro::{action_dispatch, action_impl};
use futures::Future;
use futures_util::future::join_all;
use reth_rpc_types::Log;
use sorella_db_providers::node_providers::ethereum::EthProvider;
use sorella_db_tracing::log;
use sorella_db_types::ethereum::raw::pools::{ContractProtocol, PoolDB};

use crate::raw::pools::{
    abis::{
        CurveV1MetapoolFactory::{self, BasePoolAdded as V1BasePoolAdded, MetaPoolDeployed as V1MetaPoolDeployed},
        CurveV2MetapoolFactory::{
            self, BasePoolAdded as V2BasePoolAdded, MetaPoolDeployed as V2MetaPoolDeployed,
            PlainPoolDeployed as V2PlainPoolDeployed
        },
        CurvecrvUSDFactory::{
            self, BasePoolAdded as crvUSDBasePoolAdded, MetaPoolDeployed as crvUSDMetaPoolDeployed,
            PlainPoolDeployed as crvUSDPlainPoolDeployed
        },
        Transfer
    },
    impls::{ActionCollection, FactoryDecoder},
    utils::{get_log_from_tx, rpc_to_alloy_log},
    RawEthNewPoolsResults, CURVE_BASE_POOLS_TOKENS
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

                            Some(PoolDB::new(protocol_clone, pool_addr, evt.coins.to_vec(), None, block_number))
                        }
                    })
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
                            let base_pool_addr: reth_primitives::Address = evt.base_pool.clone().0 .0.into();
                            let mut pool_tokens = CURVE_BASE_POOLS_TOKENS
                                .get(&base_pool_addr)
                                .unwrap()
                                .clone();
                            pool_tokens.push(evt.coin.0 .0.into());
                            pool_tokens.sort();

                            let Some(pool_addr) = pool_addr_future.await else { return None };

                            Some(PoolDB::new(
                                protocol_clone,
                                pool_addr,
                                pool_tokens
                                    .into_iter()
                                    .map(|token| token.0 .0.into())
                                    .collect(),
                                None,
                                block_number
                            ))
                        }
                    })
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

                PoolDB::new(
                    $protocol.clone(),
                    evt.base_pool.0 .0.into(),
                    pool_tokens
                        .into_iter()
                        .map(|token| token.0 .0.into())
                        .collect(),
                    None,
                    block_number
                )
            })
            .collect::<Vec<_>>()
    };
}

action_impl!(
    CurveV1MetapoolBaseDecoder,
    CurveV1MetapoolFactory,
    BasePoolAdded,
    true,
    false,
    |protocol: ContractProtocol, decoded_events: Vec<(alloy_primitives::Log<V1BasePoolAdded>, u64)>| {
        curve_base_pool!(protocol, decoded_events)
    }
);

action_impl!(
    CurveV1MetapoolMetaDecoder,
    CurveV1MetapoolFactory,
    MetaPoolDeployed,
    false,
    true,
    |node_handle: &'a dyn EthProvider,
     protocol: ContractProtocol,
     decoded_events: Vec<(
        alloy_primitives::Log<V1MetaPoolDeployed>,
        u64,
        Pin<Box<dyn Future<Output = Option<Address>> + Send>>
    )>| { curve_meta_pool!(protocol, decoded_events) }
);

action_impl!(
    CurveV2MetapoolBaseDecoder,
    CurveV2MetapoolFactory,
    BasePoolAdded,
    true,
    false,
    |protocol: ContractProtocol, decoded_events: Vec<(alloy_primitives::Log<V2BasePoolAdded>, u64)>| {
        curve_base_pool!(protocol, decoded_events)
    }
);

action_impl!(
    CurveV2MetapoolMetaDecoder,
    CurveV2MetapoolFactory,
    MetaPoolDeployed,
    false,
    true,
    |node_handle: &'a dyn EthProvider,
     protocol: ContractProtocol,
     decoded_events: Vec<(
        alloy_primitives::Log<V2MetaPoolDeployed>,
        u64,
        Pin<Box<dyn Future<Output = Option<Address>> + Send>>
    )>| { curve_meta_pool!(protocol, decoded_events) }
);

action_impl!(
    CurveV2MetapoolPlainDecoder,
    CurveV2MetapoolFactory,
    PlainPoolDeployed,
    false,
    true,
    |node_handle: &'a dyn EthProvider,
     protocol: ContractProtocol,
     decoded_events: Vec<(
        alloy_primitives::Log<V2PlainPoolDeployed>,
        u64,
        Pin<Box<dyn Future<Output = Option<Address>> + Send>>
    )>| { curve_plain_pool!(protocol, decoded_events) }
);

action_impl!(CurvecrvUSDBaseDecoder, CurvecrvUSDFactory, BasePoolAdded, true, false, |protocol: ContractProtocol,
                                                                                      decoded_events: Vec<(
    alloy_primitives::Log<crvUSDBasePoolAdded>,
    u64
)>| {
    curve_base_pool!(protocol, decoded_events)
});

action_impl!(
    CurvecrvUSDMetaDecoder,
    CurvecrvUSDFactory,
    MetaPoolDeployed,
    false,
    true,
    |node_handle: &'a dyn EthProvider,
     protocol: ContractProtocol,
     decoded_events: Vec<(
        alloy_primitives::Log<crvUSDMetaPoolDeployed>,
        u64,
        Pin<Box<dyn Future<Output = Option<Address>> + Send>>
    )>| { curve_meta_pool!(protocol, decoded_events) }
);

action_impl!(
    CurvecrvUSDPlainDecoder,
    CurvecrvUSDFactory,
    PlainPoolDeployed,
    false,
    true,
    |node_handle: &'a dyn EthProvider,
     protocol: ContractProtocol,
     decoded_events: Vec<(
        alloy_primitives::Log<crvUSDPlainPoolDeployed>,
        u64,
        Pin<Box<dyn Future<Output = Option<Address>> + Send>>
    )>| { curve_plain_pool!(protocol, decoded_events) }
);

action_dispatch!(
    CurveDecoder,
    CurveV1MetapoolBaseDecoder,
    CurveV2MetapoolBaseDecoder,
    CurveV2MetapoolMetaDecoder,
    CurveV2MetapoolPlainDecoder,
    CurvecrvUSDBaseDecoder,
    CurvecrvUSDMetaDecoder,
    CurvecrvUSDPlainDecoder
);
