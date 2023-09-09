use crate::{enum_unwrap, IntoAction};
use poirot_core::{
    StaticReturnBindings,
    SushiSwap_V2::{
        burnCall, mintCall, swapCall, Burn, Mint, SushiSwap_V2Calls, SushiSwap_V2Events, Swap,
    },
};

use alloy_sol_types::{SolCall, SolEvent};
use poirot_types::normalized_actions::Actions;
use reth_primitives::{Address, Bytes, Log, H160, U256};
use std::collections::HashMap;

static UNI_V2_ADDR_TO_TOKENS: HashMap<Address, (Address, Address)> = HashMap::default();

pub struct V2SwapImpl;
// event Mint(address indexed sender, uint amount0, uint amount1);
// event Burn(address indexed sender, uint amount0, uint amount1, address indexed to);
// event Swap(
//     address indexed sender,
//     uint amount0In,
//     uint amount1In,
//     uint amount0Out,
//     uint amount1Out,
//     address indexed to
// );

impl IntoAction for V2SwapImpl {
    fn decode_trace_data(
        &self,
        data: StaticReturnBindings,
        mut return_data: Bytes,
        address: Address,
        logs: &Vec<Log>,
    ) -> Actions {
        let (token_0, token_1) = UNI_V2_ADDR_TO_TOKENS.get(&address).copied().unwrap();

        for log in logs {
            if let Ok(data) = Swap::decode_data(&log.data, true) {
                let (amount_0_in, amount_1_in, amount_0_out, amount_1_out) = data;
                let amount_0_in: U256 = amount_0_in.into();

                if amount_0_in == U256::ZERO {
                    return Actions::Swap(poirot_types::normalized_actions::NormalizedSwap {
                        call_address: address,
                        token_in: token_1,
                        token_out: token_0,
                        amount_in: amount_1_in.into(),
                        amount_out: amount_0_out.into(),
                    })
                } else {
                    return Actions::Swap(poirot_types::normalized_actions::NormalizedSwap {
                        call_address: address,
                        token_in: token_0,
                        token_out: token_1,
                        amount_in: amount_0_in.into(),
                        amount_out: amount_1_out.into(),
                    })
                }
            }
        }
        unreachable!()
    }
}

pub struct V2MintImpl;
impl IntoAction for V2MintImpl {
    fn decode_trace_data(
        &self,
        data: StaticReturnBindings,
        mut return_data: Bytes,
        address: Address,
        logs: &Vec<Log>,
    ) -> Actions {
        let data = enum_unwrap!(data, SushiSwap_V2, mintCall);
        let to = H160(*data.to.0);
        let (token_0, token_1) = UNI_V2_ADDR_TO_TOKENS.get(&address).copied().unwrap();
        for log in logs {
            if let Ok((amount_0, amount_1)) = Mint::decode_data(&log.data, true) {
                return Actions::Mint(poirot_types::normalized_actions::NormalizedMint {
                    to,
                    token: vec![token_0, token_1],
                    amount: vec![amount_0.into(), amount_1.into()],
                })
            }
        }
        unreachable!()
    }
}

pub struct V2BurnImpl;
impl IntoAction for V2BurnImpl {
    fn decode_trace_data(
        &self,
        data: StaticReturnBindings,
        mut return_data: Bytes,
        address: Address,
        logs: &Vec<Log>,
    ) -> Actions {
        let data = enum_unwrap!(data, SushiSwap_V2, burnCall);
        let to = H160(*data.to.0);

        let (token_0, token_1) = UNI_V2_ADDR_TO_TOKENS.get(&address).copied().unwrap();
        for log in logs {
            if let Ok((amount_0, amount_1)) = Burn::decode_data(&log.data, true) {
                return Actions::Burn(poirot_types::normalized_actions::NormalizedBurn {
                    from: address,
                    token: vec![token_0, token_1],
                    amount: vec![amount_0.into(), amount_1.into()],
                })
            }
        }
        unreachable!()
    }
}
