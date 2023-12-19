pub mod errors;
pub mod factory;
pub mod lazy;
pub mod uniswap_v2;
pub mod uniswap_v3;
pub mod uniswap_v3_math;

use std::sync::Arc;

use alloy_primitives::{Address, Log, U256};
use alloy_sol_types::SolCall;
use async_trait::async_trait;
use brontes_types::{normalized_actions::Actions, traits::TracingProvider};
use reth_rpc_types::{CallInput, CallRequest};

use super::errors::{AmmError, ArithmeticError, EventLogError, SwapSimulationError};

async fn make_call_request<C: SolCall, T: TracingProvider>(
    call: C,
    provider: Arc<T>,
    to: Address,
    block: Option<u64>,
) -> eyre::Result<C::Return> {
    let encoded = call.abi_encode();
    let req =
        CallRequest { to: Some(to), input: CallInput::new(encoded.into()), ..Default::default() };

    let res = provider
        .eth_call(req, block.map(Into::into), None, None)
        .await?;

    Ok(C::abi_decode_returns(&res, false)?)
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
