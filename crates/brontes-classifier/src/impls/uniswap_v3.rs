use alloy_sol_types::SolCall;
use brontes_core::{
    StaticReturnBindings,
    UniswapV3::{burnCall, collectCall, mintCall, swapCall, UniswapV3Calls},
};
use brontes_types::normalized_actions::{
    Actions, NormalizedBurn, NormalizedCollect, NormalizedMint, NormalizedSwap,
};
use reth_primitives::{Address, Bytes, H160, U256};
use reth_rpc_types::Log;

use crate::{enum_unwrap, IntoAction, ADDRESS_TO_TOKENS_2_POOL};

#[derive(Debug, Default)]
pub struct V3SwapImpl;
impl IntoAction for V3SwapImpl {
    fn get_signature(&self) -> [u8; 4] {
        swapCall::SELECTOR
    }

    fn decode_trace_data(
        &self,
        index: u64,
        _data: StaticReturnBindings,
        mut return_data: Bytes,
        address: Address,
        _logs: &Vec<Log>,
    ) -> Actions {
        let return_data = swapCall::abi_decode_returns(&mut return_data, true).unwrap();
        let token_0_delta = return_data.amount0;
        let token_1_delta = return_data.amount1;
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL.get(&*address).copied().unwrap();
        let (amount_in, amount_out, token_in, token_out) = if token_0_delta.is_negative() {
            (
                U256::from_be_bytes(token_1_delta.to_be_bytes::<32>()),
                U256::from_be_bytes(token_0_delta.abs().to_be_bytes::<32>()),
                token_1,
                token_0,
            )
        } else {
            (
                U256::from_be_bytes(token_0_delta.to_be_bytes::<32>()),
                U256::from_be_bytes(token_1_delta.abs().to_be_bytes::<32>()),
                token_0,
                token_1,
            )
        };

        Actions::Swap(NormalizedSwap {
            index,
            from: address,
            pool: address,
            token_in,
            token_out,
            amount_in,
            amount_out,
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
        index: u64,
        _data: StaticReturnBindings,
        return_data: Bytes,
        address: Address,
        _logs: &Vec<Log>,
    ) -> Actions {
        let return_data = burnCall::abi_decode_returns(&return_data, true).unwrap();
        let token_0_delta: U256 = return_data.amount0;
        let token_1_delta: U256 = return_data.amount1;
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL.get(&*address).copied().unwrap();

        Actions::Burn(NormalizedBurn {
            to: address,
            recipient: address,
            index,
            from: address,
            token: vec![token_0, token_1],
            amount: vec![token_0_delta, token_1_delta],
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
        index: u64,
        _data: StaticReturnBindings,
        return_data: Bytes,
        address: Address,
        _logs: &Vec<Log>,
    ) -> Actions {
        let return_data = mintCall::abi_decode_returns(&return_data, true).unwrap();
        let token_0_delta = return_data.amount0;
        let token_1_delta = return_data.amount1;
        let [token0, token1] = ADDRESS_TO_TOKENS_2_POOL.get(&*address).copied().unwrap();

        Actions::Mint(NormalizedMint {
            index,
            from: address,
            recipient: address,
            to: address,
            token: vec![token0, token1],
            amount: vec![token_0_delta, token_1_delta],
        })
    }
}

#[derive(Debug, Default)]
pub struct V3CollectImpl;
impl IntoAction for V3CollectImpl {
    fn get_signature(&self) -> [u8; 4] {
        collectCall::SELECTOR
    }

    fn decode_trace_data(
        &self,
        index: u64,
        _data: StaticReturnBindings,
        return_data: Bytes,
        address: Address,
        _logs: &Vec<Log>,
    ) -> Actions {
        let data = enum_unwrap!(_data, UniswapV3, mintCall);
        let recipient = H160(*data.recipient.0);
        let return_data = collectCall::abi_decode_returns(&return_data, true).unwrap();
        let collect0 = return_data.amount0;
        let collect1 = return_data.amount1;
        let [token0, token1] = ADDRESS_TO_TOKENS_2_POOL.get(&*address).copied().unwrap();

        Actions::Collect(NormalizedCollect {
            index,
            from: address,
            recipient,
            to: address,
            token: vec![token0, token1],
            amount: vec![U256::from(collect0), U256::from(collect1)],
        })
    }
}
