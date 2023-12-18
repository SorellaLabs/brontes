pub mod errors;
pub mod factory;
pub mod lazy;
pub mod uniswap_v2;
pub mod uniswap_v3;
pub mod uniswap_v3_math;

use std::sync::Arc;

use alloy_primitives::{Address, B256, U256};
use alloy_sol_types::SolCall;
use async_trait::async_trait;
use brontes_types::{normalized_actions::Actions, traits::TracingProvider};
use ethers::types::Log;
use reth_rpc_types::{CallInput, CallRequest};
use serde::{Deserialize, Serialize};

use self::{uniswap_v2::UniswapV2Pool, uniswap_v3::UniswapV3Pool};
use super::errors::{AMMError, ArithmeticError, EventLogError, SwapSimulationError};

async fn make_call_request<C: SolCall, T: TracingProvider>(
    call: C,
    provider: Arc<T>,
    to: Address,
    block: u64,
) -> C::Return {
    let encoded = call.abi_encode();
    let req =
        CallRequest { to: Some(to), input: CallInput::new(encoded.into()), ..Default::default() };

    let res = provider
        .eth_call(req, Some(block.into()), None, None)
        .await
        .unwrap();
    C::abi_decode_returns(&res, false).unwrap()
}

#[async_trait]
pub trait AutomatedMarketMaker {
    fn address(&self) -> Address;
    // fn sync_on_event_signatures(&self) -> Vec<B256>;
    fn tokens(&self) -> Vec<Address>;
    fn calculate_price(&self, base_token: Address) -> Result<f64, ArithmeticError>;
    fn sync_from_action(&mut self, action: Actions) -> Result<(), EventLogError>;
    fn sync_from_log(&mut self, log: Log) -> Result<(), EventLogError>;
    async fn populate_data<M: TracingProvider>(
        &mut self,
        block_number: Option<u64>,
        middleware: Arc<M>,
    ) -> Result<(), AmmError>;

    fn simulate_swap(
        &self,
        token_in: Address,
        amount_in: U256,
    ) -> Result<U256, SwapSimulationError>;
    fn simulate_swap_mut(
        &mut self,
        token_in: Address,
        amount_in: U256,
    ) -> Result<U256, SwapSimulationError>;
    fn get_token_out(&self, token_in: Address) -> Address;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AMM {
    UniswapV2Pool(UniswapV2Pool),
    UniswapV3Pool(UniswapV3Pool),
}

#[async_trait]
impl AutomatedMarketMaker for AMM {
    fn address(&self) -> Address {
        match self {
            AMM::UniswapV2Pool(pool) => pool.address,
            AMM::UniswapV3Pool(pool) => pool.address,
        }
    }

    fn sync_from_action(&mut self, action: Actions) -> Result<(), EventLogError> {
        match self {
            AMM::UniswapV2Pool(pool) => pool.sync_from_action(action),
            AMM::UniswapV3Pool(pool) => pool.sync_from_action(action),
        }
    }

    fn sync_from_log(&mut self, log: Log) -> Result<(), EventLogError> {
        match self {
            AMM::UniswapV2Pool(pool) => pool.sync_from_log(log),
            AMM::UniswapV3Pool(pool) => pool.sync_from_log(log),
        }
    }

    fn simulate_swap(
        &self,
        token_in: Address,
        amount_in: U256,
    ) -> Result<U256, SwapSimulationError> {
        match self {
            AMM::UniswapV2Pool(pool) => pool.simulate_swap(token_in, amount_in),
            AMM::UniswapV3Pool(pool) => pool.simulate_swap(token_in, amount_in),
        }
    }

    fn simulate_swap_mut(
        &mut self,
        token_in: Address,
        amount_in: U256,
    ) -> Result<U256, SwapSimulationError> {
        match self {
            AMM::UniswapV2Pool(pool) => pool.simulate_swap_mut(token_in, amount_in),
            AMM::UniswapV3Pool(pool) => pool.simulate_swap_mut(token_in, amount_in),
        }
    }

    fn get_token_out(&self, token_in: Address) -> Address {
        match self {
            AMM::UniswapV2Pool(pool) => pool.get_token_out(token_in),
            AMM::UniswapV3Pool(pool) => pool.get_token_out(token_in),
        }
    }

    async fn populate_data<M: TracingProvider>(
        &mut self,
        block_number: Option<u64>,
        middleware: Arc<M>,
    ) -> Result<(), AmmError> {
        match self {
            AMM::UniswapV2Pool(pool) => pool.populate_data(None, middleware).await,
            AMM::UniswapV3Pool(pool) => pool.populate_data(block_number, middleware).await,
        }
    }

    fn tokens(&self) -> Vec<Address> {
        match self {
            AMM::UniswapV2Pool(pool) => pool.tokens(),
            AMM::UniswapV3Pool(pool) => pool.tokens(),
        }
    }

    fn calculate_price(&self, base_token: Address) -> Result<f64, ArithmeticError> {
        match self {
            AMM::UniswapV2Pool(pool) => pool.calculate_price(base_token),
            AMM::UniswapV3Pool(pool) => pool.calculate_price(base_token),
        }
    }
}
