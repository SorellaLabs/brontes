use alloy_primitives::{Address, Bytes, U256};
use alloy_sol_types::{SolCall, SolEvent};
use brontes_macros::{action_dispatch, action_impl};
use brontes_types::normalized_actions::{Actions, NormalizedBurn, NormalizedMint, NormalizedSwap};
use reth_rpc_types::Log;

use crate::{
    enum_unwrap, ActionCollection, IntoAction, StaticReturnBindings,
    SushiSwapV2::{burnCall, mintCall, swapCall, Burn, Mint, SushiSwapV2Calls, Swap},
    ADDRESS_TO_TOKENS_2_POOL,
};

action_impl!(
    V2SwapImpl,
    Swap,
    swapCall,
    SushiSwapV2,
    logs: true,
    |index, from_address: Address, target_address: Address, data: Option<Swap>| {
        let data = data?;
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL.get(&*target_address.0).copied()?;
        let amount_0_in: U256 = data.amount0In;
        if amount_0_in == U256::ZERO {
            return Some(NormalizedSwap {
                pool: target_address,
                index,
                from: from_address,
                token_in: token_1,
                token_out: token_0,
                amount_in: data.amount1In,
                amount_out: data.amount0Out,
            })
        } else {
            return Some(NormalizedSwap {
                index,
                pool: target_address,
                from: from_address,
                token_in: token_0,
                token_out: token_1,
                amount_in: data.amount0In,
                amount_out: data.amount1Out,
            })
        }
    }
);
action_impl!(
    V2MintImpl,
    Mint,
    mintCall,
    SushiSwapV2,
    logs: true,
    call_data: true,
    |index,
     from_address: Address,
     target_address: Address,
     call_data: mintCall,
     log_data: Option<Mint>| {
        let log_data = log_data?;
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL.get(&*target_address.0).copied()?;
        Some(NormalizedMint {
            recipient: call_data.to,
            from: from_address,
            index,
            to: target_address,
            token: vec![token_0, token_1],
            amount: vec![log_data.amount0, log_data.amount1],
        })
    }
);
action_impl!(
    V2BurnImpl,
    Burn,
    burnCall,
    SushiSwapV2,
    call_data: true,
    logs: true,
    |index,
     from_address: Address,
     target_address: Address,
     call_data: burnCall,
     log_data: Option<Burn>| {
        let log_data = log_data?;
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL.get(&*target_address.0).copied()?;
        Some(NormalizedBurn {
            recipient: call_data.to,
            to: target_address,
            index,
            from: from_address,
            token: vec![token_0, token_1],
            amount: vec![log_data.amount0, log_data.amount1],
        })
    }
);

action_dispatch!(SushiSwapV2Classifier, V2SwapImpl, V2BurnImpl, V2MintImpl);
