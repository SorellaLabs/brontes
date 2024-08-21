pub mod errors;
pub mod lazy;
pub mod uniswap_v2;
pub mod uniswap_v3;

use std::{future::Future, sync::Arc};

use alloy_primitives::{Address, Log};
use async_trait::async_trait;
use brontes_types::{normalized_actions::Action, pair::Pair, traits::TracingProvider};
pub use brontes_types::{queries::make_call_request, Protocol};
use malachite::Rational;
use tracing::warn;

use crate::{
    lazy::{PoolFetchError, PoolFetchSuccess},
    protocols::errors::{AmmError, ArithmeticError},
    types::PairWithFirstPoolHop,
    uniswap_v2::UniswapV2Pool,
    uniswap_v3::UniswapV3Pool,
    LoadResult, PoolState,
};

#[async_trait]
pub trait UpdatableProtocol {
    fn address(&self) -> Address;
    fn tokens(&self) -> Vec<Address>;
    fn calculate_price(&self, base_token: Address) -> Result<Rational, ArithmeticError>;
    fn sync_from_action(&mut self, action: Action) -> Result<(), AmmError>;
    fn sync_from_log(&mut self, log: Log) -> Result<(), AmmError>;
}

pub trait LoadState {
    fn has_state_updater(&self) -> bool;
    fn try_load_state<T: TracingProvider>(
        self,
        address: Address,
        provider: Arc<T>,
        block_number: u64,
        pool_pair: Pair,
        full_pair: PairWithFirstPoolHop,
    ) -> impl Future<Output = Result<PoolFetchSuccess, PoolFetchError>> + Send;
}

impl LoadState for Protocol {
    fn has_state_updater(&self) -> bool {
        matches!(
            self,
            Self::UniswapV2
                | Self::UniswapV3
                | Self::SushiSwapV2
                | Self::SushiSwapV3
                | Self::PancakeSwapV2
        )
    }

    async fn try_load_state<T: TracingProvider>(
        self,
        address: Address,
        provider: Arc<T>,
        block_number: u64,
        pool_pair: Pair,
        fp: PairWithFirstPoolHop,
    ) -> Result<PoolFetchSuccess, PoolFetchError> {
        match self {
            Self::UniswapV2 | Self::SushiSwapV2 | Self::PancakeSwapV2 => {
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
                                (address, Protocol::UniswapV2, block_number, pool_pair, fp, e)
                            })?,
                        LoadResult::PoolInitOnBlock,
                    )
                };

                Ok((
                    block_number,
                    address,
                    PoolState::new(
                        crate::types::PoolVariants::UniswapV2(Box::new(pool)),
                        block_number,
                    ),
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
                                (address, Protocol::UniswapV3, block_number, pool_pair, fp, e)
                            })?,
                        LoadResult::PoolInitOnBlock,
                    )
                };

                Ok((
                    block_number,
                    address,
                    PoolState::new(
                        crate::types::PoolVariants::UniswapV3(Box::new(pool)),
                        block_number,
                    ),
                    res,
                ))
            }
            rest => {
                warn!(protocol=?rest, "no state updater is build for");
                Err((address, self, block_number, pool_pair, fp, AmmError::UnsupportedProtocol))
            }
        }
    }
}
