use std::{collections::HashMap, sync::Arc};

use alloy_primitives::{Address, FixedBytes, Log};
use alloy_sol_macro::sol;
use alloy_sol_types::SolEvent;
use brontes_core::missing_token_info::load_missing_token_info;
use brontes_database::libmdbx::{DBWriter, LibmdbxReader};
use brontes_pricing::types::DexPriceMsg;
use brontes_types::{
    normalized_actions::{pool::NormalizedNewPool, Action, MultiFrameRequest},
    structured_trace::{TraceActions, TransactionTraceWithLogs, TxTrace},
    traits::TracingProvider,
    tree::{root::NodeData, GasDetails, Node, Root},
    Protocol,
};
use futures::future::join_all;
use reth_rpc_types::trace::parity::{Action as TraceAction, CallType};
use tracing::{error, trace};

use self::erc20::try_decode_transfer;
use crate::{
    classifiers::*, tree_builder::utils::decode_transfer, ActionCollection,
    FactoryDiscoveryDispatch,
};

sol!(
    #![sol(all_derives)]
    BalancerV2,
    "./classifier-abis/balancer/BalancerV2Vault.json"
);

sol!(
    #![sol(all_derives)]
    UniswapV2,
    "./classifier-abis/UniswapV2Factory.json"
);
sol!(
    #![sol(all_derives)]
    UniswapV3,
    "./classifier-abis/UniswapV3Factory.json"
);

sol!(
    #![sol(all_derives)]
    UniswapV4,
    "./classifier-abis/UniswapV4.json"
);
sol!(
    #![sol(all_derives)]
    CamelotV3,
    "./classifier-abis/Algebra1_9Factory.json"
);
sol!(
    #![sol(all_derives)]
    FluidDEX,
    "./classifier-abis/fluid/FluidDexFactory.json"
);

fn convert_to_address(address: FixedBytes<32>) -> Address {
    Address::from_slice(&address[..20])
}

pub fn decode_event(log: &Log) -> eyre::Result<(Address, Vec<Address>)> {
    // Attempt to decode the log using different protocols
    if let Ok(decoded) = BalancerV2::TokensRegistered::decode_log(log, true) {
        let pool_address = convert_to_address(decoded.poolId);
        let tokens = decoded.tokens.clone();
        Ok((pool_address, tokens))
    } else if let Ok(decoded) = UniswapV2::PairCreated::decode_log(log, true) {
        let pool_address = decoded.pair;
        let tokens = vec![decoded.token0, decoded.token1];
        Ok((pool_address, tokens))
    } else if let Ok(decoded) = UniswapV3::PoolCreated::decode_log(log, true) {
        let pool_address = decoded.pool;
        let tokens = vec![decoded.token0, decoded.token1];
        Ok((pool_address, tokens))
    } else if let Ok(decoded) = UniswapV4::Initialize::decode_log(log, true) {
        let pool_address = convert_to_address(decoded.id);
        let tokens =
            vec![convert_to_address(decoded.currency0), convert_to_address(decoded.currency1)];
        Ok((pool_address, tokens))
    } else if let Ok(decoded) = CamelotV3::Pool::decode_log(log, true) {
        let pool_address = decoded.pool;
        let tokens = vec![decoded.token0, decoded.token1];
        Ok((pool_address, tokens))
    } else if let Ok(decoded) = FluidDEX::DexT1Deployed::decode_log(log, true) {
        let pool_address = decoded.dex;
        let tokens = vec![decoded.supplyToken, decoded.borrowToken];
        Ok((pool_address, tokens))
    } else {
        println!("Failed to decode log: {:?}", log);
        Err(eyre::eyre!("Failed to decode log"))
    }
}
#[derive(Debug)]
pub struct DiscoveryLogsOnlyClassifier<'db, DB: LibmdbxReader + DBWriter> {
    libmdbx: &'db DB,
}

impl<'db, DB: LibmdbxReader + DBWriter> Clone for DiscoveryLogsOnlyClassifier<'db, DB> {
    fn clone(&self) -> Self {
        Self { libmdbx: self.libmdbx }
    }
}

impl<'db, DB: LibmdbxReader + DBWriter> DiscoveryLogsOnlyClassifier<'db, DB> {
    pub fn new(libmdbx: &'db DB) -> Self {
        Self { libmdbx }
    }

    pub async fn run_discovery(&self, block_number: u64, logs: HashMap<Protocol, Vec<Log>>) {
        self.process_logs(block_number, logs).await;
    }

    pub(crate) async fn process_logs(&self, block_number: u64, logs: HashMap<Protocol, Vec<Log>>) {
        join_all(logs.into_iter().map(|(protocol, logs)| async move {
            self.process_classification(block_number, protocol, logs)
                .await;
        }))
        .await;
    }

    async fn process_classification(&self, block_number: u64, protocol: Protocol, logs: Vec<Log>) {
        // TODO: add classification for each factory protocol and pair
        join_all(
            logs.into_iter()
                .filter_map(|log| match decode_event(&log) {
                    Ok((pool_address, tokens)) => {
                        Some(NormalizedNewPool { trace_index: 0, protocol, pool_address, tokens })
                    }
                    Err(_) => None,
                })
                .filter(|pool| !self.contains_pool(pool.pool_address))
                .map(|pool| async move { self.insert_new_pool(block_number, pool).await }),
        )
        .await;
    }

    fn contains_pool(&self, address: Address) -> bool {
        self.libmdbx.get_protocol(address).is_ok()
    }

    async fn insert_new_pool(&self, block: u64, pool: NormalizedNewPool) {
        if self
            .libmdbx
            .insert_pool(block, pool.pool_address, &pool.tokens, None, pool.protocol)
            .await
            .is_err()
        {
            error!(pool=?pool.pool_address,"failed to insert discovered pool into libmdbx");
        } else {
            trace!(
                "Discovered new {} pool:
                            \nAddress:{}
                            ",
                pool.protocol,
                pool.pool_address
            );
        }
    }
}
