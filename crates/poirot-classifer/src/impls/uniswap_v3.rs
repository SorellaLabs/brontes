use alloy_sol_types::SolCall;
use hex_literal::hex;
use poirot_core::{
    StaticReturnBindings,
    Uniswap_V3::{burnCall, mintCall, swapCall}
};
use poirot_types::normalized_actions::Actions;
use reth_primitives::{Address, Bytes, U256};
use reth_rpc_types::Log;

use crate::{IntoAction, ADDRESS_TO_TOKENS_2_POOL};

#[derive(Debug, Default)]
pub struct V3SwapImpl;
impl IntoAction for V3SwapImpl {
    fn get_signature(&self) -> [u8; 4] {
        swapCall::SELECTOR
    }

    fn decode_trace_data(
        &self,
        _data: StaticReturnBindings,
        mut return_data: Bytes,
        address: Address,
        _logs: &Vec<Log>
    ) -> Actions {
        let return_data = swapCall::decode_returns(&mut return_data, true).unwrap();
        let token_0_delta = return_data.amount0;
        let token_1_delta = return_data.amount1;
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL.get(&*address).copied().unwrap();
        let (amount_in, amount_out, token_in, token_out) = if token_0_delta.is_negative() {
            (
                U256::from_be_bytes(token_1_delta.to_be_bytes::<32>()),
                U256::from_be_bytes(token_0_delta.abs().to_be_bytes::<32>()),
                token_1,
                token_0
            )
        } else {
            (
                U256::from_be_bytes(token_0_delta.to_be_bytes::<32>()),
                U256::from_be_bytes(token_1_delta.abs().to_be_bytes::<32>()),
                token_0,
                token_1
            )
        };

        Actions::Swap(poirot_types::normalized_actions::NormalizedSwap {
            call_address: address,
            token_in,
            token_out,
            amount_in,
            amount_out
        })
    }
}

#[derive(Debug, Default)]
pub struct V3BurnImpl;
impl IntoAction for V3BurnImpl {
    fn get_signature(&self) -> [u8; 4] {
        burnCall::SELECTOR
    }

    fn decode_trace_data(
        &self,
        _data: StaticReturnBindings,
        return_data: Bytes,
        address: Address,
        _logs: &Vec<Log>
    ) -> Actions {
        let return_data = burnCall::decode_returns(&return_data, true).unwrap();
        let token_0_delta: U256 = return_data.amount0;
        let token_1_delta: U256 = return_data.amount1;
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL.get(&*address).copied().unwrap();

        Actions::Burn(poirot_types::normalized_actions::NormalizedBurn {
            from:   address,
            token:  vec![token_0, token_1],
            amount: vec![token_0_delta, token_1_delta]
        })
    }
}

#[derive(Debug, Default)]
pub struct V3MintImpl;
impl IntoAction for V3MintImpl {
    fn get_signature(&self) -> [u8; 4] {
        mintCall::SELECTOR
    }

    fn decode_trace_data(
        &self,
        _data: StaticReturnBindings,
        return_data: Bytes,
        address: Address,
        _logs: &Vec<Log>
    ) -> Actions {
        let return_data = mintCall::decode_returns(&return_data, true).unwrap();
        let token_0_delta = return_data.amount0;
        let token_1_delta = return_data.amount1;
        let [token0, token1] = ADDRESS_TO_TOKENS_2_POOL.get(&*address).copied().unwrap();

        Actions::Mint(poirot_types::normalized_actions::NormalizedMint {
            to:     address,
            token:  vec![token0, token1],
            amount: vec![token_0_delta, token_1_delta]
        })
    }
}
