use crate::{enum_unwrap, IntoAction};
use alloy_sol_types::SolCall;
use poirot_core::{
    StaticReturnBindings,
    Uniswap_V3::{burnCall, mintCall, swapCall, Uniswap_V3Calls},
};
use poirot_types::normalized_actions::Actions;
use reth_primitives::{Address, Bytes, Log, U256};
use std::collections::HashMap;

/// TODO: JOSEPH
static UNI_V3_ADDR_TO_TOKENS: HashMap<Address, (Address, Address)> = HashMap::default();

pub struct V3SwapImpl;
impl IntoAction for V3SwapImpl {
    fn decode_trace_data(
        &self,
        _data: StaticReturnBindings,
        mut return_data: Bytes,
        address: Address,
        _logs: &Vec<Log>,
    ) -> Actions {
        let return_data = swapCall::decode_returns(&mut return_data, true).unwrap();
        let token_0_delta = return_data.amount0;
        let token_1_delta = return_data.amount1;
        let (token_0, token_1) = UNI_V3_ADDR_TO_TOKENS.get(&address).copied().unwrap();
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

        Actions::Swap(poirot_types::normalized_actions::NormalizedSwap {
            call_address: address,
            token_in,
            token_out,
            amount_in,
            amount_out,
        })
    }
}

pub struct V3BurnImpl;
impl IntoAction for V3BurnImpl {
    fn decode_trace_data(
        &self,
        _data: StaticReturnBindings,
        return_data: Bytes,
        address: Address,
        _logs: &Vec<Log>,
    ) -> Actions {
        let return_data = burnCall::decode_returns(&return_data, true).unwrap();
        let token_0_delta: U256 = return_data.amount0.into();
        let token_1_delta: U256 = return_data.amount1.into();
        let (token_0, token_1) = UNI_V3_ADDR_TO_TOKENS.get(&address).copied().unwrap();

        Actions::Burn(poirot_types::normalized_actions::NormalizedBurn {
            from: address,
            token: vec![token_0, token_1],
            amount: vec![token_0_delta, token_1_delta],
        })
    }
}

pub struct V3MintImpl;
impl IntoAction for V3MintImpl {
    fn decode_trace_data(
        &self,
        _data: StaticReturnBindings,
        return_data: Bytes,
        address: Address,
        _logs: &Vec<Log>,
    ) -> Actions {
        let return_data = mintCall::decode_returns(&return_data, true).unwrap();
        let token_0_delta = return_data.amount0.into();
        let token_1_delta = return_data.amount1.into();
        let (token0, token1) = UNI_V3_ADDR_TO_TOKENS.get(&address).copied().unwrap();

        Actions::Mint(poirot_types::normalized_actions::NormalizedMint {
            to: address,
            token: vec![token0, token1],
            amount: vec![token_0_delta, token_1_delta],
        })
    }
}
