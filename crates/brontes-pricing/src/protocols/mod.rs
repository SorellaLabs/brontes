pub mod errors;
pub mod lazy;
pub mod uniswap_v2;
pub mod uniswap_v3;
// pub mod uniswap_v3_math;

use std::{future::Future, sync::Arc};

use alloy_primitives::{Address, Log};
use async_trait::async_trait;
use brontes_types::{normalized_actions::Actions, pair::Pair, traits::TracingProvider};
pub use brontes_types::{queries::make_call_request, Protocol};
use malachite::Rational;
use tracing::error;

use crate::{
    lazy::{PoolFetchError, PoolFetchSuccess},
    protocols::errors::{AmmError, ArithmeticError, EventLogError},
    uniswap_v2::UniswapV2Pool,
    uniswap_v3::UniswapV3Pool,
    LoadResult, PoolState,
};

#[async_trait]
pub trait UpdatableProtocol {
    fn address(&self) -> Address;
    fn tokens(&self) -> Vec<Address>;
    fn calculate_price(&self, base_token: Address) -> Result<Rational, ArithmeticError>;
    fn sync_from_action(&mut self, action: Actions) -> Result<(), EventLogError>;
    fn sync_from_log(&mut self, log: Log) -> Result<(), EventLogError>;
}

pub trait LoadState {
    fn try_load_state<T: TracingProvider>(
        self,
        address: Address,
        provider: Arc<T>,
        block_number: u64,
        pool_pair: Pair,
    ) -> impl Future<Output = Result<PoolFetchSuccess, PoolFetchError>> + Send;
}

impl LoadState for Protocol {
    async fn try_load_state<T: TracingProvider>(
        self,
        address: Address,
        provider: Arc<T>,
        block_number: u64,
        pool_pair: Pair,
    ) -> Result<PoolFetchSuccess, PoolFetchError> {
        match self {
            Self::UniswapV2 | Self::SushiSwapV2 => {
                let (pool, res) = if let Ok(pool) =
                    UniswapV2Pool::new_load_on_block(address, provider.clone(), block_number - 1)
                        .await
                {
                    (pool, LoadResult::Ok)
                } else {
                    (
                        UniswapV2Pool::new_load_on_block(address, provider, block_number)
                            .await
                            .map_err(|e| {
                                (address, Protocol::UniswapV2, block_number, pool_pair, e)
                            })?,
                        LoadResult::PoolInitOnBlock,
                    )
                };

                Ok((
                    block_number,
                    address,
                    PoolState::new(crate::types::PoolVariants::UniswapV2(pool), block_number),
                    res,
                ))
            }
            Self::UniswapV3 | Self::SushiSwapV3 => {
                let (pool, res) = if let Ok(pool) =
                    UniswapV3Pool::new_from_address(address, block_number - 1, provider.clone())
                        .await
                {
                    (pool, LoadResult::Ok)
                } else {
                    (
                        UniswapV3Pool::new_from_address(address, block_number, provider)
                            .await
                            .map_err(|e| {
                                (address, Protocol::UniswapV3, block_number, pool_pair, e)
                            })?,
                        LoadResult::PoolInitOnBlock,
                    )
                };

                Ok((
                    block_number,
                    address,
                    PoolState::new(crate::types::PoolVariants::UniswapV3(pool), block_number),
                    res,
                ))
            }
            rest => {
                error!(protocol=?rest, "no state updater is build for");
                Err((address, self, block_number, pool_pair, AmmError::UnsupportedProtocol))
            }
        }
    }
}
