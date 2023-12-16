use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use async_trait::async_trait;
use ethers::{
    abi::RawLog,
    prelude::{abigen, EthEvent},
    providers::Middleware,
    types::{BlockNumber, Filter, Log, H160, H256, U256, U64},
};
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;

use crate::{
    amm::{
        factory::{AutomatedMarketMakerFactory, TASK_LIMIT},
        AutomatedMarketMaker, AMM,
    },
    errors::{AMMError, EventLogError},
};

use super::{batch_request, UniswapV3Pool, BURN_EVENT_SIGNATURE, MINT_EVENT_SIGNATURE};

abigen!(
    IUniswapV3Factory,
    r#"[
        function getPool(address tokenA, address tokenB, uint24 fee) external view returns (address pool)
        event PoolCreated(address indexed token0, address indexed token1, uint24 indexed fee, int24 tickSpacing, address pool)
        function parameters() returns (address, address, uint24, int24)
        function feeAmountTickSpacing(uint24) returns (int24)
        ]"#;
);

pub const POOL_CREATED_EVENT_SIGNATURE: H256 = H256([
    120, 60, 202, 28, 4, 18, 221, 13, 105, 94, 120, 69, 104, 201, 109, 162, 233, 194, 47, 249, 137,
    53, 122, 46, 139, 29, 155, 43, 78, 107, 113, 24,
]);

#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct UniswapV3Factory {
    pub address: H160,
    pub creation_block: u64,
}

#[async_trait]
impl AutomatedMarketMakerFactory for UniswapV3Factory {
    fn address(&self) -> H160 {
        self.address
    }

    fn creation_block(&self) -> u64 {
        self.creation_block
    }

    fn amm_created_event_signature(&self) -> H256 {
        POOL_CREATED_EVENT_SIGNATURE
    }

    async fn new_amm_from_log<M: 'static + Middleware>(
        &self,
        log: Log,
        middleware: Arc<M>,
    ) -> Result<AMM, AMMError<M>> {
        if let Some(block_number) = log.block_number {
            let pool_created_filter = PoolCreatedFilter::decode_log(&RawLog::from(log))?;
            Ok(AMM::UniswapV3Pool(
                UniswapV3Pool::new_from_address(
                    pool_created_filter.pool,
                    block_number.as_u64(),
                    middleware,
                )
                .await?,
            ))
        } else {
            return Err(AMMError::BlockNumberNotFound);
        }
    }

    async fn get_all_amms<M: 'static + Middleware>(
        &self,
        to_block: Option<u64>,
        middleware: Arc<M>,
        step: u64,
    ) -> Result<Vec<AMM>, AMMError<M>> {
        if let Some(block) = to_block {
            self.get_all_pools_from_logs(block, step, middleware).await
        } else {
            return Err(AMMError::BlockNumberNotFound);
        }
    }

    async fn populate_amm_data<M: Middleware>(
        &self,
        amms: &mut [AMM],
        block_number: Option<u64>,
        middleware: Arc<M>,
    ) -> Result<(), AMMError<M>> {
        if let Some(block_number) = block_number {
            let step = 127; //Max batch size for call
            for amm_chunk in amms.chunks_mut(step) {
                batch_request::get_amm_data_batch_request(
                    amm_chunk,
                    block_number,
                    middleware.clone(),
                )
                .await?;
            }
        } else {
            return Err(AMMError::BlockNumberNotFound);
        }

        Ok(())
    }

    fn new_empty_amm_from_log(&self, log: Log) -> Result<AMM, ethers::abi::Error> {
        let pool_created_event = PoolCreatedFilter::decode_log(&RawLog::from(log))?;

        Ok(AMM::UniswapV3Pool(UniswapV3Pool {
            address: pool_created_event.pool,
            token_a: pool_created_event.token_0,
            token_b: pool_created_event.token_1,
            token_a_decimals: 0,
            token_b_decimals: 0,
            fee: pool_created_event.fee,
            liquidity: 0,
            sqrt_price: U256::zero(),
            tick_spacing: 0,
            tick: 0,
            tick_bitmap: HashMap::new(),
            ticks: HashMap::new(),
        }))
    }
}

impl UniswapV3Factory {
    pub fn new(address: H160, creation_block: u64) -> UniswapV3Factory {
        UniswapV3Factory {
            address,
            creation_block,
        }
    }

    //Function to get all pair created events for a given Dex factory address and sync pool data
    pub async fn get_all_pools_from_logs<M: 'static + Middleware>(
        self,
        to_block: u64,
        step: u64,
        middleware: Arc<M>,
    ) -> Result<Vec<AMM>, AMMError<M>> {
        //Unwrap can be used here because the creation block was verified within `Dex::new()`
        let mut from_block = self.creation_block;
        let mut aggregated_amms: HashMap<H160, AMM> = HashMap::new();
        let mut ordered_logs: BTreeMap<U64, Vec<Log>> = BTreeMap::new();

        tracing::info!(from_block, to_block, step, "getting all pools from logs");

        let mut handles = vec![];

        let mut tasks = 0;
        while from_block < to_block {
            let middleware = middleware.clone();

            let mut target_block = from_block + step - 1;
            if target_block > to_block {
                target_block = to_block;
            }

            handles.push(tokio::spawn(async move {
                let logs = middleware
                    .get_logs(
                        &Filter::new()
                            .topic0(vec![
                                POOL_CREATED_EVENT_SIGNATURE,
                                BURN_EVENT_SIGNATURE,
                                MINT_EVENT_SIGNATURE,
                            ])
                            .from_block(BlockNumber::Number(U64([from_block])))
                            .to_block(BlockNumber::Number(U64([target_block]))),
                    )
                    .await
                    .map_err(AMMError::MiddlewareError)?;

                Ok::<Vec<Log>, AMMError<M>>(logs)
            }));

            from_block += step;

            tasks += 1;
            //Here we are limiting the number of green threads that can be spun up to not have the node time out
            if tasks == TASK_LIMIT {
                self.process_logs_from_handles(handles, &mut ordered_logs)
                    .await?;
                handles = vec![];
                tasks = 0;
            }
        }

        self.process_logs_from_handles(handles, &mut ordered_logs)
            .await?;

        for (_, log_group) in ordered_logs {
            for log in log_group {
                let event_signature = log.topics[0];

                //If the event sig is the pool created event sig, then the log is coming from the factory
                if event_signature == POOL_CREATED_EVENT_SIGNATURE {
                    if log.address == self.address {
                        let mut new_pool = self.new_empty_amm_from_log(log)?;

                        if let AMM::UniswapV3Pool(ref mut pool) = new_pool {
                            pool.tick_spacing = pool.get_tick_spacing(middleware.clone()).await?;
                        }

                        aggregated_amms.insert(new_pool.address(), new_pool);
                    }
                } else if event_signature == BURN_EVENT_SIGNATURE {
                    //If the event sig is the BURN_EVENT_SIGNATURE log is coming from the pool
                    if let Some(AMM::UniswapV3Pool(pool)) = aggregated_amms.get_mut(&log.address) {
                        pool.sync_from_burn_log(log)?;
                    }
                } else if event_signature == MINT_EVENT_SIGNATURE {
                    if let Some(AMM::UniswapV3Pool(pool)) = aggregated_amms.get_mut(&log.address) {
                        pool.sync_from_mint_log(log)?;
                    }
                }
            }
        }

        Ok(aggregated_amms.into_values().collect::<Vec<AMM>>())
    }

    async fn process_logs_from_handles<M: Middleware>(
        &self,
        handles: Vec<JoinHandle<Result<Vec<Log>, AMMError<M>>>>,
        ordered_logs: &mut BTreeMap<U64, Vec<Log>>,
    ) -> Result<(), AMMError<M>> {
        // group the logs from each thread by block number and then sync the logs in chronological order
        for handle in handles {
            let logs = handle.await??;

            for log in logs {
                if let Some(log_block_number) = log.block_number {
                    if let Some(log_group) = ordered_logs.get_mut(&log_block_number) {
                        log_group.push(log);
                    } else {
                        ordered_logs.insert(log_block_number, vec![log]);
                    }
                } else {
                    return Err(EventLogError::LogBlockNumberNotFound)?;
                }
            }
        }
        Ok(())
    }
}
