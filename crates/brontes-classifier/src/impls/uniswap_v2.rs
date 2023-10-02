use alloy_sol_types::{SolCall, SolEvent};
use brontes_core::{
    StaticReturnBindings,
    SushiSwap_V2::{burnCall, mintCall, swapCall, Burn, Mint, SushiSwap_V2Calls, Swap},
};
use brontes_types::normalized_actions::{Actions, NormalizedBurn, NormalizedMint, NormalizedSwap};
use reth_primitives::{Address, Bytes, H160, U256};
use reth_rpc_types::Log;

use crate::{enum_unwrap, IntoAction, ADDRESS_TO_TOKENS_2_POOL};

#[derive(Debug, Default)]
pub struct V2SwapImpl;

impl IntoAction for V2SwapImpl {
    fn get_signature(&self) -> [u8; 4] {
        swapCall::SELECTOR
    }

    fn decode_trace_data(
        &self,
        index: u64,
        _data: StaticReturnBindings,
        _return_data: Bytes,
        address: Address,
        logs: &Vec<Log>,
    ) -> Actions {
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL.get(&*address).copied().unwrap();

        for log in logs {
            if let Ok(data) = Swap::decode_log(log.topics.iter().map(|h| h.0), &log.data, true) {
                let amount_0_in: U256 = data.amount0In;

                if amount_0_in == U256::ZERO {
                    return Actions::Swap(NormalizedSwap {
                        pool: address,
                        index,
                        from: address,
                        token_in: token_1,
                        token_out: token_0,
                        amount_in: data.amount1In,
                        amount_out: data.amount0Out,
                    })
                } else {
                    return Actions::Swap(NormalizedSwap {
                        index,
                        pool: address,
                        from: address,
                        token_in: token_0,
                        token_out: token_1,
                        amount_in: data.amount0In,
                        amount_out: data.amount1Out,
                    })
                }
            }
        }
        unreachable!()
    }
}

#[derive(Debug, Default)]
pub struct V2MintImpl;
impl IntoAction for V2MintImpl {
    fn get_signature(&self) -> [u8; 4] {
        mintCall::SELECTOR
    }

    fn decode_trace_data(
        &self,
        index: u64,
        data: StaticReturnBindings,
        _return_data: Bytes,
        address: Address,
        logs: &Vec<Log>,
    ) -> Actions {
        let data = enum_unwrap!(data, SushiSwap_V2, mintCall);
        let to = H160(*data.to.0);
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL.get(&*address).copied().unwrap();
        for log in logs {
            if let Ok(res) = Mint::decode_log(log.topics.iter().map(|h| h.0), &log.data, true) {
                return Actions::Mint(NormalizedMint {
                    recipient: address,
                    from: address,
                    index,
                    to,
                    token: vec![token_0, token_1],
                    amount: vec![res.amount0, res.amount1],
                })
            }
        }
        unreachable!()
    }
}

#[derive(Debug, Default)]
pub struct V2BurnImpl;
impl IntoAction for V2BurnImpl {
    fn get_signature(&self) -> [u8; 4] {
        burnCall::SELECTOR
    }

    fn decode_trace_data(
        &self,
        index: u64,
        _data: StaticReturnBindings,
        _return_data: Bytes,
        address: Address,
        logs: &Vec<Log>,
    ) -> Actions {
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL.get(&*address).copied().unwrap();
        for log in logs {
            if let Ok(res) = Burn::decode_log(log.topics.iter().map(|h| h.0), &log.data, true) {
                return Actions::Burn(NormalizedBurn {
                    recipient: address,
                    to: address,
                    index,
                    from: address,
                    token: vec![token_0, token_1],
                    amount: vec![res.amount0, res.amount1],
                })
            }
        }
        unreachable!()
    }
}
