use std::{collections::HashMap, sync::Arc};

use alloy_primitives::{Address, FixedBytes};
use alloy_rpc_types::Log;
use alloy_sol_macro::sol;
use alloy_sol_types::SolEvent;
use brontes_database::libmdbx::{DBWriter, LibmdbxReader};
use brontes_types::{
    constants::FLUID_VAULT_RESOLVER_ADDRESS, normalized_actions::pool::NormalizedNewPool,
    traits::TracingProvider, Protocol,
};
use futures::future::join_all;
use tracing::{debug, error};

use crate::{
    query_fluid_lending_market_tokens, query_pendle_v2_market_tokens, ActionCollection,
    FactoryDiscoveryDispatch, FluidVaultFactory,
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
sol!(
    #![sol(all_derives)]
    PendleV2Market,
    "./classifier-abis/pendle_v2/PendleMarketFactoryV3.json"
);
sol!(
    #![sol(all_derives)]
    PendleV2YieldFactory,
    "./classifier-abis/pendle_v2/PendleYieldContractFactory.json"
);

fn convert_to_address(address: FixedBytes<32>) -> Address {
    Address::from_slice(&address[12..])
}

pub async fn decode_event<T: TracingProvider>(
    protocol: Protocol,
    plog: &alloy_primitives::Log,
    tracer: Arc<T>,
) -> eyre::Result<(Address, Vec<Address>)> {
    let (pool_address, tokens) = match protocol {
        Protocol::UniswapV2
        | Protocol::SushiSwapV2
        | Protocol::PancakeSwapV2
        | Protocol::CamelotV2 => {
            let decoded = UniswapV2::PairCreated::decode_log(plog, true)?;
            (decoded.pair, vec![decoded.token0, decoded.token1])
        }
        Protocol::UniswapV3 | Protocol::SushiSwapV3 | Protocol::PancakeSwapV3 => {
            let decoded = UniswapV3::PoolCreated::decode_log(plog, true)?;
            (decoded.pool, vec![decoded.token0, decoded.token1])
        }
        Protocol::UniswapV4 => {
            let decoded = UniswapV4::Initialize::decode_log(plog, true)?;
            (
                Address::from_slice(&decoded.id[..20]),
                vec![convert_to_address(decoded.currency0), convert_to_address(decoded.currency1)],
            )
        }
        Protocol::CamelotV3 => {
            let decoded = CamelotV3::Pool::decode_log(plog, true)?;
            (decoded.pool, vec![decoded.token0, decoded.token1])
        }
        Protocol::FluidDEX => {
            let decoded = FluidDEX::DexT1Deployed::decode_log(plog, true)?;
            (decoded.dex, vec![decoded.supplyToken, decoded.borrowToken])
        }
        Protocol::FluidLending => {
            let decoded = FluidVaultFactory::VaultDeployed::decode_log(plog, true)?;
            let vault = decoded.vault;
            let tokens =
                query_fluid_lending_market_tokens(&tracer, &vault, FLUID_VAULT_RESOLVER_ADDRESS)
                    .await;
            (vault, tokens)
        }
        Protocol::LFJV2_1 => {
            let decoded = LFJV2::LBPairCreated::decode_log(plog, true)?;
            (decoded.LBPair, vec![decoded.tokenX, decoded.tokenY])
        }
        Protocol::BalancerV2 => {
            let decoded = BalancerV2::TokensRegistered::decode_log(plog, true)?;
            (Address::from_slice(&decoded.poolId[..20]), decoded.tokens.clone())
        }
        Protocol::PendleV2 => {
            if let Ok(decoded) = PendleV2Market::CreateNewMarket::decode_log(plog, true) {
                let tokens = query_pendle_v2_market_tokens(&tracer, &decoded.market).await;
                (decoded.market, tokens)
            } else if let Ok(decoded) =
                PendleV2YieldFactory::CreateYieldContract::decode_log(plog, true)
            {
                (decoded.YT, vec![decoded.SY, decoded.PT, decoded.YT])
            } else {
                return Err(eyre::eyre!("Unsupported Pendle event: {:?}", plog));
            }
        }
        _ => {
            return Err(eyre::eyre!("Unsupported protocol: {:?}", protocol));
        }
    };
    Ok((pool_address, tokens))
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

    pub async fn process_logs<T: TracingProvider>(
        &self,
        logs: HashMap<Protocol, Vec<Log>>,
        tracer: Arc<T>,
    ) -> eyre::Result<()> {
        let futures = logs
            .into_iter()
            .map(|(protocol, logs)| {
                let tracer_clone = tracer.clone();
                async move {
                    self.process_classification(protocol, logs, tracer_clone)
                        .await;
                }
            })
            .collect::<Vec<_>>();
        join_all(futures).await;
        Ok(())
    }

    async fn process_classification<T: TracingProvider>(
        &self,
        protocol: Protocol,
        logs: Vec<Log>,
        tracer: Arc<T>,
    ) {
        let decoded_events_futures = logs.into_iter().map(|log| {
            let tracer_clone = tracer.clone();
            async move {
                match decode_event(protocol, &log.inner, tracer_clone).await {
                    Ok((pool_address, tokens)) => Some((
                        log.block_number,
                        NormalizedNewPool { trace_index: 0, protocol, pool_address, tokens },
                    )),
                    Err(e) => {
                        tracing::debug!("Failed to decode event: {:?}", e);
                        None
                    }
                }
            }
        });

        let results = join_all(decoded_events_futures).await;

        let new_pools_to_insert_futures = results
            .into_iter()
            .filter_map(|opt_decoded_event| opt_decoded_event) // Filter out None results from decoding
            .filter(|(_, pool)| !self.contains_pool(pool.pool_address)) // pool.pool_address should now be accessible
            .map(|(block_number, pool)| async move {
                self.insert_new_pool(block_number, pool, None).await
            });

        join_all(new_pools_to_insert_futures).await;
    }

    fn contains_pool(&self, address: Address) -> bool {
        let protocol = self.libmdbx.get_protocol(address).ok();
        if let Some(protocol) = protocol {
            tracing::debug!(
                "already contains_pool: {:?} address {}",
                protocol.into_clickhouse_protocol(),
                address
            );
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

#[cfg(all(test))]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{Address, B256};
    use alloy_sol_types::SolEvent;

    use super::*;
    // Helper function to create a mock address from hex string
    fn mock_address(hex: &str) -> Address {
        Address::from_str(hex).unwrap()
    }

    fn create_mock_uniswap_v3_pool_created_log(
        factory_address: Address,
        token0: Address,
        token1: Address,
        fee: u32, // fee is in basis points (e.g., 3000 for 0.3%)
        pool: Address,
    ) -> alloy_primitives::Log {
        // Create the event data
        let event = UniswapV3::PoolCreated {
            token0,
            token1,
            fee,
            pool,
            tickSpacing: 60, // typical tick spacing for most fee tiers
        };

        // Encode the event to get topics and data
        let topics: Vec<B256> = event
            .encode_topics()
            .iter()
            .map(|t| B256::from_slice(t.as_slice()))
            .collect();
        let data: alloy_primitives::Bytes = event.encode_data().into();

        // Create the log
        alloy_primitives::Log::new(factory_address, topics, data).unwrap()
    }

    #[test]
    fn test_decode_uniswap_v3_pool_created() -> eyre::Result<()> {
        let factory = mock_address("0x1F98431c8aD98523631AE4a59f267346ea31F984");
        let token0 = mock_address("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
        let token1 = mock_address("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
        let pool = mock_address("0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640");
        let fee = 3000; // 0.3% fee tier

        let log = create_mock_uniswap_v3_pool_created_log(factory, token0, token1, fee, pool);

        let decoded = UniswapV3::PoolCreated::decode_log(&log, true)?;
        assert_eq!(decoded.pool, pool); // pool address
        assert_eq!(decoded.token0, token0); // tokens
        assert_eq!(decoded.token1, token1); // tokens

        let (decoded_pool, decoded_tokens) =
            decode_event(Protocol::UniswapV3, &log, Arc::new(MockTracingProvider::new()))?;
        assert_eq!(decoded_pool, pool); // pool address
        assert_eq!(decoded_tokens, vec![token0, token1]); // tokens

        Ok(()) // Return Ok(()) to indicate test success
    }
}
