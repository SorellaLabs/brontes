use std::sync::Arc;


use brontes_types::traits::TracingProvider;
use async_trait::async_trait;
use ethers::{
    types::{BlockNumber, Filter, Log, ValueOrArray, H160, H256, U64},
};
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;

use super::{
    errors::{AMMError, EventLogError},
    uniswap_v2::factory::{UniswapV2Factory, PAIR_CREATED_EVENT_SIGNATURE},
    uniswap_v3::factory::{UniswapV3Factory, POOL_CREATED_EVENT_SIGNATURE},
    AMM,
};

pub const TASK_LIMIT: usize = 10;

#[async_trait]
pub trait AutomatedMarketMakerFactory {
    fn address(&self) -> H160;

    async fn get_all_amms<M: 'static + TracingProvider>(
        &self,
        to_block: Option<u64>,
        middleware: Arc<M>,
        step: u64,
    ) -> Result<Vec<AMM>, AMMError<M>>;

    async fn populate_amm_data<M: TracingProvider>(
        &self,
        amms: &mut [AMM],
        block_number: Option<u64>,
        middleware: Arc<M>,
    ) -> Result<(), AMMError<M>>;

    fn amm_created_event_signature(&self) -> H256;

    fn creation_block(&self) -> u64;

    async fn new_amm_from_log<M: 'static + TracingProvider>(
        &self,
        log: Log,
        middleware: Arc<M>,
    ) -> Result<AMM, AMMError<M>>;

    fn new_empty_amm_from_log(&self, log: Log) -> Result<AMM, ethers::abi::Error>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Factory {
    UniswapV2Factory(UniswapV2Factory),
    UniswapV3Factory(UniswapV3Factory),
}

#[async_trait]
impl AutomatedMarketMakerFactory for Factory {
    fn address(&self) -> H160 {
        match self {
            Factory::UniswapV2Factory(factory) => factory.address(),
            Factory::UniswapV3Factory(factory) => factory.address(),
        }
    }

    fn amm_created_event_signature(&self) -> H256 {
        match self {
            Factory::UniswapV2Factory(factory) => factory.amm_created_event_signature(),
            Factory::UniswapV3Factory(factory) => factory.amm_created_event_signature(),
        }
    }

    async fn new_amm_from_log<M: 'static + TracingProvider>(
        &self,
        log: Log,
        middleware: Arc<M>,
    ) -> Result<AMM, AMMError<M>> {
        match self {
            Factory::UniswapV2Factory(factory) => factory.new_amm_from_log(log, middleware).await,
            Factory::UniswapV3Factory(factory) => factory.new_amm_from_log(log, middleware).await,
        }
    }

    fn new_empty_amm_from_log(&self, log: Log) -> Result<AMM, ethers::abi::Error> {
        match self {
            Factory::UniswapV2Factory(factory) => factory.new_empty_amm_from_log(log),
            Factory::UniswapV3Factory(factory) => factory.new_empty_amm_from_log(log),
        }
    }

    async fn get_all_amms<M: 'static + TracingProvider>(
        &self,
        to_block: Option<u64>,
        middleware: Arc<M>,
        step: u64,
    ) -> Result<Vec<AMM>, AMMError<M>> {
        match self {
            Factory::UniswapV2Factory(factory) => {
                factory.get_all_amms(to_block, middleware, step).await
            }
            Factory::UniswapV3Factory(factory) => {
                factory.get_all_amms(to_block, middleware, step).await
            }
        }
    }

    async fn populate_amm_data<M: TracingProvider>(
        &self,
        amms: &mut [AMM],
        block_number: Option<u64>,
        middleware: Arc<M>,
    ) -> Result<(), AMMError<M>> {
        match self {
            Factory::UniswapV2Factory(factory) => {
                factory.populate_amm_data(amms, None, middleware).await
            }
            Factory::UniswapV3Factory(factory) => {
                factory
                    .populate_amm_data(amms, block_number, middleware)
                    .await
            }
        }
    }

    fn creation_block(&self) -> u64 {
        match self {
            Factory::UniswapV2Factory(uniswap_v2_factory) => uniswap_v2_factory.creation_block,
            Factory::UniswapV3Factory(uniswap_v3_factory) => uniswap_v3_factory.creation_block,
        }
    }
}

impl Factory {
    // pub async fn get_all_pools_from_logs<M: 'static + TracingProvider>(
    //     &self,
    //     mut from_block: u64,
    //     to_block: u64,
    //     step: u64,
    //     middleware: Arc<M>,
    // ) -> Result<Vec<AMM>, AMMError<M>> {
    //     let factory_address = self.address();
    //     let amm_created_event_signature = self.amm_created_event_signature();
    //     let mut log_group = vec![];
    //     let mut handles = vec![];
    //     let mut tasks = 0;
    //     let mut aggregated_amms: Vec<AMM> = vec![];
    //
    //     while from_block < to_block {
    //         let middleware = middleware.clone();
    //         let mut target_block = from_block + step - 1;
    //         if target_block > to_block {
    //             target_block = to_block;
    //         }
    //
    //         handles.push(tokio::spawn(async move {
    //             let logs = middleware
    //                 .get_logs(
    //                     &Filter::new()
    //                         .topic0(ValueOrArray::Value(amm_created_event_signature))
    //                         .address(factory_address)
    //                         .from_block(BlockNumber::Number(U64([from_block])))
    //                         .to_block(BlockNumber::Number(U64([target_block]))),
    //                 )
    //                 .await
    //                 .map_err(AMMError::TracingProviderError)?;
    //
    //             Ok::<Vec<Log>, AMMError<M>>(logs)
    //         }));
    //
    //         from_block += step;
    //         tasks += 1;
    //         if tasks == TASK_LIMIT {
    //             self.process_logs_from_handles(handles, &mut log_group)
    //                 .await?;
    //
    //             handles = vec![];
    //             tasks = 0;
    //         }
    //     }
    //
    //     self.process_logs_from_handles(handles, &mut log_group)
    //         .await?;
    //
    //     for log in log_group {
    //         aggregated_amms.push(self.new_empty_amm_from_log(log)?);
    //     }
    //
    //     Ok(aggregated_amms)
    // }
    //
    // async fn process_logs_from_handles<M: TracingProvider>(
    //     &self,
    //     handles: Vec<JoinHandle<Result<Vec<Log>, AMMError<M>>>>,
    //     log_group: &mut Vec<Log>,
    // ) -> Result<(), AMMError<M>> {
    //     for handle in handles {
    //         let logs = handle.await??;
    //         for log in logs {
    //             log_group.push(log);
    //         }
    //     }
    //     Ok(())
    // }
}

impl TryFrom<H256> for Factory {
    type Error = EventLogError;

    fn try_from(value: H256) -> Result<Self, Self::Error> {
        if value == PAIR_CREATED_EVENT_SIGNATURE {
            Ok(Factory::UniswapV2Factory(UniswapV2Factory::default()))
        } else if value == POOL_CREATED_EVENT_SIGNATURE {
            Ok(Factory::UniswapV3Factory(UniswapV3Factory::default()))
        } else {
            return Err(EventLogError::InvalidEventSignature)
        }
    }
}
