pub mod errors;
pub mod factory;
pub mod lazy;
pub mod uniswap_v2;
pub mod uniswap_v3;

use std::sync::Arc;

use amms::errors::{AMMError, ArithmeticError, EventLogError,
SwapSimulationError}; use async_trait::async_trait;
use ethers::{
    providers::Middleware,
    types::{Log, H160, H256, U256},
};
use serde::{Deserialize, Serialize};

use self::{uniswap_v2::UniswapV2Pool, uniswap_v3::UniswapV3Pool};

#[async_trait]
pub trait AutomatedMarketMaker {
    fn address(&self) -> H160;
    async fn sync<M: Middleware>(&mut self, middleware: Arc<M>) -> Result<(),
AMMError<M>>;     fn sync_on_event_signatures(&self) -> Vec<H256>;
    fn tokens(&self) -> Vec<H160>;
    fn calculate_price(&self, base_token: H160) -> Result<f64,
ArithmeticError>;     fn sync_from_log(&mut self, log: Log) -> Result<(),
EventLogError>;     async fn populate_data<M: Middleware>(
        &mut self,
        block_number: Option<u64>,
        middleware: Arc<M>,
    ) -> Result<(), AMMError<M>>;

    fn simulate_swap(&self, token_in: H160, amount_in: U256) -> Result<U256,
SwapSimulationError>;     fn simulate_swap_mut(
        &mut self,
        token_in: H160,
        amount_in: U256,
    ) -> Result<U256, SwapSimulationError>;
    fn get_token_out(&self, token_in: H160) -> H160;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AMM {
    UniswapV2Pool(UniswapV2Pool),
    UniswapV3Pool(UniswapV3Pool),
}
