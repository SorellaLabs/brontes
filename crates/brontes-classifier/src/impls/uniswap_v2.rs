use alloy_sol_types::{SolCall, SolEvent};
use brontes_core::{
    StaticReturnBindings,
    SushiSwap_V2::{burnCall, mintCall, swapCall, Burn, Mint, SushiSwap_V2Calls, Swap},
};
use brontes_types::normalized_actions::{Actions, NormalizedBurn, NormalizedMint, NormalizedSwap};
use reth_primitives::{Address, Bytes, H160, U256};
use reth_rpc_types::Log;

use crate::{
    action_impl_all_no_return, action_impl_log, action_impl_log_no_return, enum_unwrap, IntoAction,
    ADDRESS_TO_TOKENS_2_POOL,
};

action_impl_log_no_return!(
    V2SwapImpl,
    Swap,
    swapCall,
    Swap,
    |index, address: H160, data: &Swap| {
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL.get(&*address).copied().unwrap();
        let amount_0_in: U256 = data.amount0In;
        if amount_0_in == U256::ZERO {
            return NormalizedSwap {
                pool: address,
                index,
                from: address,
                token_in: token_1,
                token_out: token_0,
                amount_in: data.amount1In,
                amount_out: data.amount0Out,
            }
        } else {
            return NormalizedSwap {
                index,
                pool: address,
                from: address,
                token_in: token_0,
                token_out: token_1,
                amount_in: data.amount0In,
                amount_out: data.amount1Out,
            }
        }
    }
);

action_impl_log_no_return!(
    V2MintImpl,
    Mint,
    mintCall,
    Mint,
    |index, address: H160, data: &Mint| {
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL.get(&*address).copied().unwrap();
        NormalizedMint {
            recipient: address,
            from: address,
            index,
            // todo fix
            to: data.sender,
            token: vec![token_0, token_1],
            amount: vec![data.amount0, data.amount1],
        }
    }
);

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
