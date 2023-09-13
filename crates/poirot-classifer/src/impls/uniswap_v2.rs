use alloy_sol_types::{SolCall, SolEvent};
use hex_literal::hex;
use poirot_core::{
    StaticReturnBindings,
    SushiSwap_V2::{burnCall, mintCall, swapCall, Burn, Mint, SushiSwap_V2Calls, Swap}
};
use poirot_types::normalized_actions::{Actions, NormalizedBurn, NormalizedMint, NormalizedSwap};
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
        _data: StaticReturnBindings,
        _return_data: Bytes,
        address: Address,
        logs: &Vec<Log>
    ) -> Actions {
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL.get(&*address).copied().unwrap();

        for log in logs {
            if let Ok(data) = Swap::decode_data(&log.data, true) {
                let (amount_0_in, amount_1_in, amount_0_out, amount_1_out) = data;
                let amount_0_in: U256 = amount_0_in;

                if amount_0_in == U256::ZERO {
                    return Actions::Swap(NormalizedSwap {
                        call_address: address,
                        token_in:     token_1,
                        token_out:    token_0,
                        amount_in:    amount_1_in,
                        amount_out:   amount_0_out
                    })
                } else {
                    return Actions::Swap(NormalizedSwap {
                        call_address: address,
                        token_in:     token_0,
                        token_out:    token_1,
                        amount_in:    amount_0_in,
                        amount_out:   amount_1_out
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
        data: StaticReturnBindings,
        _return_data: Bytes,
        address: Address,
        logs: &Vec<Log>
    ) -> Actions {
        let data = enum_unwrap!(data, SushiSwap_V2, mintCall);
        let to = H160(*data.to.0);
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL.get(&*address).copied().unwrap();
        for log in logs {
            if let Ok((amount_0, amount_1)) = Mint::decode_data(&log.data, true) {
                return Actions::Mint(NormalizedMint {
                    to,
                    token: vec![token_0, token_1],
                    amount: vec![amount_0, amount_1]
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
        _data: StaticReturnBindings,
        _return_data: Bytes,
        address: Address,
        logs: &Vec<Log>
    ) -> Actions {
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL.get(&*address).copied().unwrap();
        for log in logs {
            if let Ok((amount_0, amount_1)) = Burn::decode_data(&log.data, true) {
                return Actions::Burn(NormalizedBurn {
                    from:   address,
                    token:  vec![token_0, token_1],
                    amount: vec![amount_0, amount_1]
                })
            }
        }
        unreachable!()
    }
}
