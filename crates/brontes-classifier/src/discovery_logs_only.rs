use std::collections::HashMap;

use alloy_primitives::{Address, FixedBytes};
use alloy_rpc_types::Log;
use alloy_sol_macro::sol;
use alloy_sol_types::SolEvent;
use brontes_database::libmdbx::{DBWriter, LibmdbxReader};
use brontes_types::{normalized_actions::pool::NormalizedNewPool, Protocol};
use futures::future::join_all;
use tracing::{debug, error};

use crate::{ActionCollection, FactoryDiscoveryDispatch};

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
    LFJV2,
    "./classifier-abis/LFJ/ILBFactory.json"
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

pub fn decode_event(log: &Log) -> eyre::Result<(Option<u64>, Address, Vec<Address>)> {
    let plog: alloy_primitives::Log = log.inner.clone();
    let (pool_address, tokens) = match (
        BalancerV2::TokensRegistered::decode_log(&plog, true),
        UniswapV2::PairCreated::decode_log(&plog, true),
        UniswapV3::PoolCreated::decode_log(&plog, true),
        UniswapV4::Initialize::decode_log(&plog, true),
        CamelotV3::Pool::decode_log(&plog, true),
        FluidDEX::DexT1Deployed::decode_log(&plog, true),
        LFJV2::LBPairCreated::decode_log(&plog, true),
    ) {
        (Ok(decoded), ..) => (convert_to_address(decoded.poolId), decoded.tokens.clone()),
        (_, Ok(decoded), ..) => (decoded.pair, vec![decoded.token0, decoded.token1]),
        (_, _, Ok(decoded), ..) => (decoded.pool, vec![decoded.token0, decoded.token1]),
        (_, _, _, Ok(decoded), ..) => (
            convert_to_address(decoded.id),
            vec![convert_to_address(decoded.currency0), convert_to_address(decoded.currency1)],
        ),
        (_, _, _, _, Ok(decoded), ..) => (decoded.pool, vec![decoded.token0, decoded.token1]),
        (_, _, _, _, _, Ok(decoded), _) => {
            (decoded.dex, vec![decoded.supplyToken, decoded.borrowToken])
        }
        (_, _, _, _, _, _, Ok(decoded)) => (decoded.LBPair, vec![decoded.tokenX, decoded.tokenY]),
        _ => {
            tracing::debug!("Failed to decode log: {:?}", plog);
            return Err(eyre::eyre!("Failed to decode log"));
        }
    };
    Ok((log.block_number, pool_address, tokens))
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

    pub async fn process_logs(&self, logs: HashMap<Protocol, Vec<Log>>) -> eyre::Result<()> {
        let _ = join_all(logs.into_iter().map(|(protocol, logs)| async move {
            self.process_classification(protocol, logs).await;
        }))
        .await;
        Ok(())
    }

    async fn process_classification(&self, protocol: Protocol, logs: Vec<Log>) {
        join_all(
            logs.into_iter()
                .filter_map(|log| {
                    decode_event(&log)
                        .map(|(block_number, pool_address, tokens)| {
                            (
                                block_number,
                                NormalizedNewPool {
                                    trace_index: 0,
                                    protocol,
                                    pool_address,
                                    tokens,
                                },
                            )
                        })
                        .ok()
                })
                .filter(|(_, pool)| !self.contains_pool(pool.pool_address))
                .map(|(block_number, pool)| async move {
                    self.insert_new_pool(block_number, pool, None).await
                }),
        )
        .await;
    }

    fn contains_pool(&self, address: Address) -> bool {
        let protocol = self.libmdbx.get_protocol(address).ok();
        if let Some(protocol) = protocol {
            tracing::debug!("already contains_pool: {:?} address {}", protocol.into_clickhouse_protocol(), address);
        }
        protocol.is_some()
    }

    async fn insert_new_pool(
        &self,
        block_number: Option<u64>,
        pool: NormalizedNewPool,
        curve_lp_token: Option<Address>,
    ) {
        tracing::debug!("insert_new_pool: {:?}", pool.pool_address);
        let insert_result = self
            .libmdbx
            .insert_pool(
                block_number.unwrap_or(0),
                pool.pool_address,
                &pool.tokens,
                curve_lp_token,
                pool.protocol,
            )
            .await;

        if insert_result.is_err() {
            error!(pool=?pool.pool_address, "failed to insert discovered pool into libmdbx");
        } else {
            debug!("Discovered new {} pool: Address:{}", pool.protocol, pool.pool_address);
        }
    }
}
